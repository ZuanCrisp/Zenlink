package dev.zenshiro.zenlink.net

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import java.net.InetAddress

/** A ZenLink receiver found on the local network. */
data class Peer(val name: String, val host: InetAddress, val port: Int, val tv: Boolean)

/**
 * mDNS/DNS-SD through Android's NsdManager (no extra library): receivers
 * register `_zenlink._tcp`, senders browse for it. A multicast lock is held
 * while browsing because some phones drop multicast with the screen on Wi-Fi
 * power save.
 */
class Discovery(context: Context) {
    private val nsd = context.getSystemService(Context.NSD_SERVICE) as NsdManager
    private val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
    private var registration: NsdManager.RegistrationListener? = null
    private var browse: NsdManager.DiscoveryListener? = null
    private var lock: WifiManager.MulticastLock? = null

    fun advertise(name: String, port: Int, tv: Boolean) {
        stopAdvertising()
        val info = NsdServiceInfo().apply {
            serviceName = name
            serviceType = SERVICE_TYPE
            setPort(port)
            setAttribute("v", SecureLink.VERSION.toString())
            setAttribute("tv", if (tv) "1" else "0")
        }
        val l = object : NsdManager.RegistrationListener {
            override fun onServiceRegistered(info: NsdServiceInfo) {}
            override fun onRegistrationFailed(info: NsdServiceInfo, code: Int) {}
            override fun onServiceUnregistered(info: NsdServiceInfo) {}
            override fun onUnregistrationFailed(info: NsdServiceInfo, code: Int) {}
        }
        registration = l
        nsd.registerService(info, NsdManager.PROTOCOL_DNS_SD, l)
    }

    fun stopAdvertising() {
        registration?.let { runCatching { nsd.unregisterService(it) } }
        registration = null
    }

    /** Browses for receivers; [onChange] gets the current list on every change. */
    fun browse(onChange: (List<Peer>) -> Unit) {
        stopBrowsing()
        lock = wifi.createMulticastLock("zenlink-discovery").apply { setReferenceCounted(false); acquire() }
        val found = linkedMapOf<String, Peer>()
        val l = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(type: String) {}
            override fun onDiscoveryStopped(type: String) {}
            override fun onStartDiscoveryFailed(type: String, code: Int) {}
            override fun onStopDiscoveryFailed(type: String, code: Int) {}
            override fun onServiceLost(info: NsdServiceInfo) {
                synchronized(found) { found.remove(info.serviceName); onChange(found.values.toList()) }
            }
            override fun onServiceFound(info: NsdServiceInfo) {
                @Suppress("DEPRECATION") // resolveService: the callback API needs Android 14
                nsd.resolveService(info, object : NsdManager.ResolveListener {
                    override fun onResolveFailed(info: NsdServiceInfo, code: Int) {}
                    override fun onServiceResolved(info: NsdServiceInfo) {
                        val host = hostOf(info) ?: return
                        val tv = info.attributes["tv"]?.let { String(it) } == "1"
                        synchronized(found) {
                            found[info.serviceName] = Peer(info.serviceName, host, info.port, tv)
                            onChange(found.values.toList())
                        }
                    }
                })
            }
        }
        browse = l
        nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, l)
    }

    fun stopBrowsing() {
        browse?.let { runCatching { nsd.stopServiceDiscovery(it) } }
        browse = null
        lock?.let { runCatching { it.release() } }
        lock = null
    }

    fun close() {
        stopAdvertising()
        stopBrowsing()
    }

    private fun hostOf(info: NsdServiceInfo): InetAddress? =
        if (Build.VERSION.SDK_INT >= 34) info.hostAddresses.firstOrNull() else @Suppress("DEPRECATION") info.host

    companion object {
        const val SERVICE_TYPE = "_zenlink._tcp."
    }
}
