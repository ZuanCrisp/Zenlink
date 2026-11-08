package dev.zenshiro.zenlink

import dev.zenshiro.zenlink.net.SecureLink
import dev.zenshiro.zenlink.net.WrongCodeException
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.IOException
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.Callable
import java.util.concurrent.Executors

class SecureLinkTest {
    // Real loopback sockets: piped streams fail once the thread that wrote last has ended.
    private fun connect(senderCode: String, receiverCode: String): Pair<Result<SecureLink>, Result<SecureLink>> {
        val server = ServerSocket(0, 1, InetAddress.getLoopbackAddress())
        val pool = Executors.newFixedThreadPool(2)
        val r = pool.submit(Callable {
            val sock = server.accept()
            runCatching { SecureLink.handshake(sock.getInputStream(), sock.getOutputStream(), SecureLink.ROLE_RECEIVER, "TV", receiverCode) }
                .also { if (it.isFailure) sock.close() }
        })
        val s = pool.submit(Callable {
            val sock = Socket(server.inetAddress, server.localPort)
            runCatching { SecureLink.handshake(sock.getInputStream(), sock.getOutputStream(), SecureLink.ROLE_SENDER, "Phone", senderCode) }
                .also { if (it.isFailure) sock.close() }
        })
        val out = s.get() to r.get()
        pool.shutdown()
        server.close()
        return out
    }

    @Test
    fun sameCodeConnectsAndRecordsRoundTrip() {
        val (s, r) = connect("123456", "123456")
        val sender = s.getOrThrow()
        val receiver = r.getOrThrow()
        assertEquals("TV", sender.peerName)
        assertEquals("Phone", receiver.peerName)
        val frame = ByteArray(100_000) { (it % 251).toByte() }
        sender.send(2, frame)
        sender.send(6, ByteArray(0))
        val got = receiver.receive()
        assertEquals(2, got.type)
        assertArrayEquals(frame, got.payload)
        assertEquals(6, receiver.receive().type)
        receiver.send(5, byteArrayOf(1, 2, 3))
        assertArrayEquals(byteArrayOf(1, 2, 3), sender.receive().payload)
    }

    @Test
    fun wrongCodeFailsOnBothEnds() {
        val (s, r) = connect("123456", "654321")
        assertTrue(s.exceptionOrNull() is IOException)
        assertTrue(r.exceptionOrNull() is WrongCodeException)
    }

    @Test
    fun codesAreSixDigits() {
        repeat(50) { assertTrue(SecureLink.newCode().matches(Regex("\\d{6}"))) }
    }
}
