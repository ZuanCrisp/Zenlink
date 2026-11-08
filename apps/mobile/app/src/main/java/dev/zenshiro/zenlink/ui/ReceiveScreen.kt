package dev.zenshiro.zenlink.ui

import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import dev.zenshiro.zenlink.MainActivity
import dev.zenshiro.zenlink.net.Discovery
import dev.zenshiro.zenlink.net.Link
import dev.zenshiro.zenlink.net.SecureLink
import dev.zenshiro.zenlink.net.Wire
import dev.zenshiro.zenlink.net.WrongCodeException
import dev.zenshiro.zenlink.tr
import java.net.ServerSocket
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread

/**
 * Receiver: listens on a free port, announces itself on the network and shows
 * a pairing code. Video playback arrives in phase 8; for now a connected
 * sender is shown and kept alive with pings.
 */
@Composable
fun ReceiveScreen(tv: Boolean, back: () -> Unit) {
    val context = LocalContext.current
    val name = remember { MainActivity.deviceName(context) }
    var code by remember { mutableStateOf(SecureLink.newCode()) }
    var status by remember { mutableStateOf(tr("Menunggu pengirim…", "Waiting for a sender…")) }
    var peer by remember { mutableStateOf<String?>(null) }

    DisposableEffect(Unit) {
        val discovery = Discovery(context)
        var server: ServerSocket? = null
        val alive = AtomicBoolean(true)
        val worker = thread(name = "zenlink-receive", isDaemon = true) {
            try {
                val s = Link.listen().also { server = it }
                discovery.advertise(name, s.localPort, tv)
                while (alive.get()) {
                    val open = try {
                        Link.accept(s, name, code)
                    } catch (e: WrongCodeException) {
                        status = tr("Kode salah. Coba lagi.", "Wrong code. Try again.")
                        continue
                    } catch (e: Exception) {
                        if (!alive.get()) break
                        status = tr("Gagal terhubung: ", "Could not connect: ") + e.message
                        continue
                    }
                    peer = open.link.peerName
                    status = tr("Terhubung", "Connected")
                    try {
                        while (alive.get()) {
                            val r = open.link.receive()
                            if (r.type == Wire.PING) open.link.send(Wire.PING, ByteArray(0))
                            if (r.type == Wire.BYE) break
                        }
                    } catch (_: Exception) {
                    } finally {
                        open.close()
                    }
                    peer = null
                    code = SecureLink.newCode() // a new code for the next sender
                    status = tr("Pengirim terputus. Menunggu pengirim…", "Sender left. Waiting for a sender…")
                }
            } catch (e: Exception) {
                if (alive.get()) status = tr("Tidak bisa membuka penerima: ", "Cannot start the receiver: ") + e.message
            }
        }
        onDispose {
            alive.set(false)
            discovery.close()
            runCatching { server?.close() }
            worker.interrupt()
        }
    }

    Page(
        title = tr("Terima layar", "Receive a screen"),
        subtitle = tr(
            "Di HP pengirim: buka ZenLink, pilih Kirim layar ini, pilih \"$name\", lalu masukkan kode ini.",
            "On the sending phone: open ZenLink, choose Send this screen, pick \"$name\", then enter this code.",
        ),
        back = if (tv) null else back,
    ) {
        ZlCard {
            Text(tr("Kode", "Code"), color = Zl.Dim)
            Text(
                code.chunked(3).joinToString(" "),
                fontSize = if (tv) 72.sp else 56.sp,
                fontWeight = FontWeight.Bold,
                fontFamily = FontFamily.Monospace,
                color = Zl.Pink,
            )
            Text(status, color = Zl.Text)
            peer?.let { Text(tr("Pengirim: ", "Sender: ") + it, color = Zl.Dim) }
        }
    }
}
