package dev.zenshiro.zenlink.net

import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket

/**
 * Opens a [SecureLink] over TCP: the receiver listens and shows a pairing code,
 * the sender connects and types it. Blocking calls; run them off the main thread.
 */
object Link {
    const val CONNECT_TIMEOUT_MS = 5_000
    const val HANDSHAKE_TIMEOUT_MS = 15_000

    class Open(val socket: Socket, val link: SecureLink) {
        fun close() = runCatching { socket.close() }
    }

    /** Receiver side: a listening socket on any free port. */
    fun listen(): ServerSocket = ServerSocket(0).apply { reuseAddress = true }

    /** Receiver side: waits for one sender and checks [code]. */
    fun accept(server: ServerSocket, name: String, code: String): Open {
        val s = server.accept()
        return handshake(s, SecureLink.ROLE_RECEIVER, name, code)
    }

    /** Sender side. */
    fun connect(host: InetAddress, port: Int, name: String, code: String): Open {
        val s = Socket()
        s.connect(InetSocketAddress(host, port), CONNECT_TIMEOUT_MS)
        return handshake(s, SecureLink.ROLE_SENDER, name, code)
    }

    private fun handshake(s: Socket, role: Int, name: String, code: String): Open {
        try {
            s.tcpNoDelay = true
            s.soTimeout = HANDSHAKE_TIMEOUT_MS
            val link = SecureLink.handshake(s.getInputStream(), s.getOutputStream(), role, name, code)
            s.soTimeout = 0
            return Open(s, link)
        } catch (e: Exception) {
            runCatching { s.close() }
            throw e
        }
    }
}
