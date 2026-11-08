/*
 * zlaac: ZenLink's tiny AAC / AAC-ELD decoder DLL.
 *
 * Wraps a minimal static FFmpeg (only the "aac" decoder and swresample, see
 * build_ffmpeg.sh) behind four C functions, so the Python backend (ctypes)
 * and later the Rust code can decode the iPhone/iPad's AAC-ELD sound without
 * shipping the full FFmpeg. Output is always interleaved s16 stereo at the
 * requested rate, the same as the old PyAV path (AudioResampler s16/stereo).
 *
 * Copyright (C) 2026 Zenvy Zenshiro. GPL-3.0-or-later.
 * FFmpeg is LGPL-2.1-or-later (see ZenshiroDev/docs/THIRD_PARTY.md).
 */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/avcodec.h>
#include <libavutil/channel_layout.h>
#include <libavutil/opt.h>
#include <libavutil/version.h>
#include <libswresample/swresample.h>

#define ZLAAC_API __declspec(dllexport)

typedef struct ZlAac {
    AVCodecContext *ctx;
    AVPacket *pkt;
    AVFrame *frame;
    SwrContext *swr;
    int out_rate;
    int swr_in_rate;
    int swr_in_fmt;
    int swr_in_channels;
} ZlAac;

ZLAAC_API void zl_aac_close(ZlAac *d)
{
    if (!d)
        return;
    swr_free(&d->swr);
    av_frame_free(&d->frame);
    av_packet_free(&d->pkt);
    avcodec_free_context(&d->ctx);
    free(d);
}

/* asc: AudioSpecificConfig (the "magic cookie"). in_rate / channels describe
 * the stream when the config alone is not enough (AAC-ELD from Apple). */
ZLAAC_API ZlAac *zl_aac_open(const uint8_t *asc, int asc_len, int in_rate, int channels, int out_rate)
{
    const AVCodec *codec = avcodec_find_decoder(AV_CODEC_ID_AAC);
    ZlAac *d;

    if (!codec || asc_len < 0 || out_rate <= 0 || channels < 1 || channels > 2)
        return NULL;
    d = calloc(1, sizeof(*d));
    if (!d)
        return NULL;
    d->out_rate = out_rate;
    d->ctx = avcodec_alloc_context3(codec);
    d->pkt = av_packet_alloc();
    d->frame = av_frame_alloc();
    if (!d->ctx || !d->pkt || !d->frame)
        goto fail;
    if (asc_len > 0) {
        d->ctx->extradata = av_mallocz(asc_len + AV_INPUT_BUFFER_PADDING_SIZE);
        if (!d->ctx->extradata)
            goto fail;
        memcpy(d->ctx->extradata, asc, asc_len);
        d->ctx->extradata_size = asc_len;
    }
    d->ctx->sample_rate = in_rate;
    av_channel_layout_default(&d->ctx->ch_layout, channels);
    if (avcodec_open2(d->ctx, codec, NULL) < 0)
        goto fail;
    return d;
fail:
    zl_aac_close(d);
    return NULL;
}

static int ensure_swr(ZlAac *d, const AVFrame *f)
{
    AVChannelLayout stereo = AV_CHANNEL_LAYOUT_STEREO;

    if (d->swr && d->swr_in_rate == f->sample_rate && d->swr_in_fmt == f->format &&
        d->swr_in_channels == f->ch_layout.nb_channels)
        return 0;
    swr_free(&d->swr);
    if (swr_alloc_set_opts2(&d->swr, &stereo, AV_SAMPLE_FMT_S16, d->out_rate, &f->ch_layout,
                            (enum AVSampleFormat)f->format, f->sample_rate, 0, NULL) < 0)
        return -1;
    if (swr_init(d->swr) < 0) {
        swr_free(&d->swr);
        return -1;
    }
    d->swr_in_rate = f->sample_rate;
    d->swr_in_fmt = f->format;
    d->swr_in_channels = f->ch_layout.nb_channels;
    return 0;
}

/* Decode one access unit. Writes up to cap stereo frames (cap * 2 int16) to
 * out and returns the number of frames written, 0 when the decoder needs more
 * input, or a negative value on error (-2: out too small). */
ZLAAC_API int zl_aac_decode(ZlAac *d, const uint8_t *au, int len, int16_t *out, int cap)
{
    int written = 0;
    int ret;

    if (!d || !au || len <= 0 || !out || cap <= 0)
        return -1;
    if (av_new_packet(d->pkt, len) < 0)
        return -1;
    memcpy(d->pkt->data, au, len);
    ret = avcodec_send_packet(d->ctx, d->pkt);
    av_packet_unref(d->pkt);
    if (ret < 0)
        return -1;
    for (;;) {
        uint8_t *dst;
        int room, n;

        ret = avcodec_receive_frame(d->ctx, d->frame);
        if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF)
            break;
        if (ret < 0)
            return -1;
        if (ensure_swr(d, d->frame) < 0) {
            av_frame_unref(d->frame);
            return -1;
        }
        room = cap - written;
        if (swr_get_out_samples(d->swr, d->frame->nb_samples) > room) {
            av_frame_unref(d->frame);
            return -2;
        }
        dst = (uint8_t *)(out + 2 * written);
        n = swr_convert(d->swr, &dst, room, (const uint8_t **)d->frame->extended_data, d->frame->nb_samples);
        av_frame_unref(d->frame);
        if (n < 0)
            return -1;
        written += n;
    }
    return written;
}

ZLAAC_API unsigned zl_aac_avcodec_version(void)
{
    return avcodec_version();
}
