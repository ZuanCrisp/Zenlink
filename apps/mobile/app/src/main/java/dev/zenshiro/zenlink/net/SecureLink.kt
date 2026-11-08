package dev.zenshiro.zenlink.net

import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.EOFException
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.nio.ByteBuffer
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.spec.ECGenParameterSpec
import java.security.spec.X509EncodedKeySpec
import javax.crypto.Cipher
import javax.crypto.KeyAgreement
import javax.crypto.Mac
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * ZenLink LAN link, version 1: an encrypted, length-framed record stream
 * between a sender and a receiver on the same network.
 *
 * Handshake (both sides send at once, then read):
 *   hello = "ZLNK" | u8 version | u8 role | u16 nameLen | name (UTF-8)
 *           | u16 keyLen | EC P-256 public key (X.509) | 16-byte nonce
 * Both derive, with HKDF-SHA256 over the ECDH secret, salt = SHA-256 of
 * (sender hello | receiver hello) and info = "zenlink-v1" | pairing code:
 *   send key and receive key (AES-256-GCM, one per direction) and a confirm key.
 * Each side then sends HMAC(confirm, role | transcript); a wrong pairing code
 * makes the confirm check fail and the link closes.
 *
 * Records: u32 length | AES-GCM(ciphertext + tag). The 12-byte nonce is a
 * per-direction counter, never sent; a record out of order fails the tag.
 * Plaintext: u8 type | payload.
 *
 * Limits (documented, not hidden): the 6-digit code protects against anyone
 * passively watching the network; an active attacker in the middle at pairing
 * time could test codes offline. A PAKE (SPAKE2) is the planned upgrade.
 */
class SecureLink private constructor(
    private val input: DataInputStream,
    private val output: DataOutputStream,
    private val sendKey: SecretKeySpec,
    private val recvKey: SecretKeySpec,
    val peerName: String,
) {
    private var sendCounter = 0L
    private var recvCounter = 0L
    private val sendLock = Any()
    private val encrypt = Cipher.getInstance("AES/GCM/NoPadding")
    private val decrypt = Cipher.getInstance("AES/GCM/NoPadding")

    /** Sends one record. Thread-safe; records keep their order. */
    fun send(type: Int, payload: ByteArray, offset: Int = 0, length: Int = payload.size) {
        synchronized(sendLock) {
            encrypt.init(Cipher.ENCRYPT_MODE, sendKey, GCMParameterSpec(128, nonce(sendCounter++)))
            encrypt.update(byteArrayOf(type.toByte()))
            val sealed = encrypt.doFinal(payload, offset, length)
            output.writeInt(sealed.size)
            output.write(sealed)
            output.flush()
        }
    }

    /** Reads the next record (blocking). Throws on a bad tag or a closed link. */
    fun receive(): Record {
        val size = input.readInt()
        if (size < 17 || size > MAX_RECORD) throw IOException("bad record size $size")
        val sealed = ByteArray(size)
        input.readFully(sealed)
        decrypt.init(Cipher.DECRYPT_MODE, recvKey, GCMParameterSpec(128, nonce(recvCounter++)))
        val plain = decrypt.doFinal(sealed)
        return Record(plain[0].toInt() and 0xff, plain.copyOfRange(1, plain.size))
    }

    class Record(val type: Int, val payload: ByteArray)

    companion object {
        const val VERSION = 1
        const val ROLE_SENDER = 1
        const val ROLE_RECEIVER = 2
        const val MAX_RECORD = 8 shl 20
        private val MAGIC = byteArrayOf('Z'.code.toByte(), 'L'.code.toByte(), 'N'.code.toByte(), 'K'.code.toByte())
        private val random = SecureRandom()

        /** A new 6-digit pairing code, shown by the receiver. */
        fun newCode(): String = String.format("%06d", random.nextInt(1_000_000))

        /**
         * Runs the handshake on a connected stream pair. [code] is the pairing
         * code shown on the receiver (typed on the sender).
         */
        fun handshake(
            inp: InputStream,
            out: OutputStream,
            role: Int,
            name: String,
            code: String,
        ): SecureLink {
            val input = DataInputStream(inp.buffered())
            val output = DataOutputStream(out.buffered())
            val kpg = KeyPairGenerator.getInstance("EC")
            kpg.initialize(ECGenParameterSpec("secp256r1"))
            val pair = kpg.generateKeyPair()
            val nonce = ByteArray(16).also(random::nextBytes)
            val mine = hello(role, name.toByteArray().let { if (it.size > 200) it.copyOf(200) else it }, pair.public.encoded, nonce)
            output.write(mine)
            output.flush()
            val theirs = readHello(input)
            if (theirs.role == role) throw IOException("both ends are the same role")

            val peerKey = KeyFactory.getInstance("EC").generatePublic(X509EncodedKeySpec(theirs.key))
            val agree = KeyAgreement.getInstance("ECDH")
            agree.init(pair.private)
            agree.doPhase(peerKey, true)
            val secret = agree.generateSecret()

            val senderHello = if (role == ROLE_SENDER) mine else theirs.raw
            val receiverHello = if (role == ROLE_SENDER) theirs.raw else mine
            val transcript = sha256(senderHello + receiverHello)
            val okm = hkdf(secret, transcript, "zenlink-v1".toByteArray() + code.toByteArray(), 96)
            val s2r = SecretKeySpec(okm.copyOfRange(0, 32), "AES")
            val r2s = SecretKeySpec(okm.copyOfRange(32, 64), "AES")
            val confirm = okm.copyOfRange(64, 96)

            output.write(hmac(confirm, byteArrayOf(role.toByte()) + transcript))
            output.flush()
            val theirConfirm = ByteArray(32)
            try {
                input.readFully(theirConfirm)
            } catch (e: EOFException) {
                throw WrongCodeException()
            }
            val expected = hmac(confirm, byteArrayOf(theirs.role.toByte()) + transcript)
            if (!MessageDigest.isEqual(expected, theirConfirm)) throw WrongCodeException()

            return if (role == ROLE_SENDER) {
                SecureLink(input, output, s2r, r2s, theirs.name)
            } else {
                SecureLink(input, output, r2s, s2r, theirs.name)
            }
        }

        private class Hello(val raw: ByteArray, val role: Int, val name: String, val key: ByteArray)

        private fun hello(role: Int, n: ByteArray, key: ByteArray, nonce: ByteArray): ByteArray {
            val b = ByteBuffer.allocate(4 + 1 + 1 + 2 + n.size + 2 + key.size + 16)
            b.put(MAGIC).put(VERSION.toByte()).put(role.toByte())
            b.putShort(n.size.toShort()).put(n)
            b.putShort(key.size.toShort()).put(key)
            b.put(nonce)
            return b.array()
        }

        private fun readHello(input: DataInputStream): Hello {
            val magic = ByteArray(4).also(input::readFully)
            if (!magic.contentEquals(MAGIC)) throw IOException("not a ZenLink device")
            val version = input.readUnsignedByte()
            if (version != VERSION) throw IOException("ZenLink link version $version is not supported; update the app")
            val role = input.readUnsignedByte()
            val nLen = input.readUnsignedShort()
            if (nLen > 200) throw IOException("bad hello")
            val name = ByteArray(nLen).also(input::readFully)
            val kLen = input.readUnsignedShort()
            if (kLen > 200) throw IOException("bad hello")
            val key = ByteArray(kLen).also(input::readFully)
            val nonce = ByteArray(16).also(input::readFully)
            val raw = hello(role, name, key, nonce)
            return Hello(raw, role, String(name), key)
        }

        private fun nonce(counter: Long): ByteArray =
            ByteBuffer.allocate(12).putInt(0).putLong(counter).array()

        private fun sha256(b: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(b)

        private fun hmac(key: ByteArray, data: ByteArray): ByteArray {
            val mac = Mac.getInstance("HmacSHA256")
            mac.init(SecretKeySpec(key, "HmacSHA256"))
            return mac.doFinal(data)
        }

        /** RFC 5869 HKDF-SHA256. */
        internal fun hkdf(ikm: ByteArray, salt: ByteArray, info: ByteArray, length: Int): ByteArray {
            val prk = hmac(salt, ikm)
            val out = java.io.ByteArrayOutputStream()
            var t = ByteArray(0)
            var i = 1
            while (out.size() < length) {
                t = hmac(prk, t + info + byteArrayOf(i.toByte()))
                out.write(t)
                i++
            }
            return out.toByteArray().copyOf(length)
        }
    }
}

class WrongCodeException : IOException("wrong pairing code")
