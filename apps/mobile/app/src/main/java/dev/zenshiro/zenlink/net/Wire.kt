package dev.zenshiro.zenlink.net

import java.nio.ByteBuffer

/** Record types inside a [SecureLink] (u8 type, then the payload). */
object Wire {
    /** Sender → receiver: u16 width, u16 height, u8 codec (1 = H.264, 2 = H.265), codec config bytes. */
    const val VIDEO_CONFIG = 1
    /** Sender → receiver: i64 presentation time (µs), u8 flags (1 = key frame), frame bytes. */
    const val VIDEO_FRAME = 2
    /** Sender → receiver: u32 sample rate, u8 channels, AAC AudioSpecificConfig. */
    const val AUDIO_CONFIG = 3
    /** Sender → receiver: i64 presentation time (µs), AAC access unit. */
    const val AUDIO_FRAME = 4
    /** Receiver → sender: u8 action (0 down, 1 move, 2 up), f32 x, f32 y in 0..1 of the picture. */
    const val TOUCH = 5
    /** Either way: keep-alive, empty. */
    const val PING = 6
    /** Receiver → sender: ask for a key frame (after a decoder reset). */
    const val KEY_FRAME_REQUEST = 7
    /** Either way: u8 reason, closing. */
    const val BYE = 8

    const val CODEC_H264 = 1
    const val CODEC_H265 = 2

    class VideoConfig(val width: Int, val height: Int, val codec: Int, val csd: ByteArray)

    fun videoConfig(c: VideoConfig): ByteArray =
        ByteBuffer.allocate(5 + c.csd.size).putShort(c.width.toShort()).putShort(c.height.toShort())
            .put(c.codec.toByte()).put(c.csd).array()

    fun parseVideoConfig(b: ByteArray): VideoConfig {
        val buf = ByteBuffer.wrap(b)
        val w = buf.short.toInt() and 0xffff
        val h = buf.short.toInt() and 0xffff
        val codec = buf.get().toInt()
        val csd = ByteArray(buf.remaining()).also { buf.get(it) }
        return VideoConfig(w, h, codec, csd)
    }

    fun touch(action: Int, x: Float, y: Float): ByteArray =
        ByteBuffer.allocate(9).put(action.toByte()).putFloat(x).putFloat(y).array()
}
