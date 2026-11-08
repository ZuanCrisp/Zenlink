// Dev-only tool: encodes a deterministic synthetic stereo signal to raw
// AAC-ELD (480-sample frames, no SBR), the format iPhones/iPads send, and
// writes a fixture for the zlaac tests:
//
//   "ZLAU" | u8 asc_len | asc | u32le rate | repeated { u16le len | access unit }
//
// Built against the Fraunhofer FDK AAC encoder, which is used only on the
// developer's PC to make the fixture. It is never bundled with ZenLink.
//
//   make_eld_fixture <rate> <seconds> <bitrate> <out.zlau>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>

#include "aacenc_lib.h"

static void put16(FILE *f, unsigned v) { fputc(v & 0xff, f); fputc((v >> 8) & 0xff, f); }

int main(int argc, char **argv)
{
    if (argc != 5) {
        fprintf(stderr, "usage: %s <rate> <seconds> <bitrate> <out.zlau>\n", argv[0]);
        return 2;
    }
    const int rate = atoi(argv[1]);
    const double seconds = atof(argv[2]);
    const int bitrate = atoi(argv[3]);
    HANDLE_AACENCODER enc;
    if (aacEncOpen(&enc, 0, 2) != AACENC_OK) return 1;
    if (aacEncoder_SetParam(enc, AACENC_AOT, AOT_ER_AAC_ELD) != AACENC_OK ||
        aacEncoder_SetParam(enc, AACENC_SBR_MODE, 0) != AACENC_OK ||
        aacEncoder_SetParam(enc, AACENC_GRANULE_LENGTH, 480) != AACENC_OK ||
        aacEncoder_SetParam(enc, AACENC_SAMPLERATE, rate) != AACENC_OK ||
        aacEncoder_SetParam(enc, AACENC_CHANNELMODE, MODE_2) != AACENC_OK ||
        aacEncoder_SetParam(enc, AACENC_BITRATE, bitrate) != AACENC_OK ||
        aacEncoder_SetParam(enc, AACENC_TRANSMUX, TT_MP4_RAW) != AACENC_OK ||
        aacEncEncode(enc, NULL, NULL, NULL, NULL) != AACENC_OK) {
        fprintf(stderr, "encoder setup failed\n");
        return 1;
    }
    AACENC_InfoStruct info = {};
    aacEncInfo(enc, &info);
    FILE *f = fopen(argv[4], "wb");
    if (!f) return 1;
    fwrite("ZLAU", 1, 4, f);
    fputc((int)info.confSize, f);
    fwrite(info.confBuf, 1, info.confSize, f);
    put16(f, rate & 0xffff); put16(f, (unsigned)rate >> 16);
    fprintf(stderr, "asc:");
    for (UINT i = 0; i < info.confSize; i++) fprintf(stderr, " %02x", info.confBuf[i]);
    fprintf(stderr, " frame=%u\n", info.frameLength);

    const int frame = (int)info.frameLength;
    const long total = (long)(rate * seconds);
    std::vector<INT_PCM> pcm(frame * 2);
    std::vector<UCHAR> out(8192);
    long t = 0;
    int aus = 0;
    unsigned noise = 12345;
    bool flushing = false;
    for (;;) {
        int n = 0;
        if (!flushing) {
            for (; n < frame && t < total; n++, t++) {
                const double s = (double)t / rate;
                const double chirp = std::sin(2 * M_PI * (200.0 + 1800.0 * s / seconds) * s);
                noise = noise * 1103515245u + 12345u;
                const double hiss = ((noise >> 16) & 0x7fff) / 32768.0 - 0.5;
                const double l = 0.45 * std::sin(2 * M_PI * 440.0 * s) + 0.2 * chirp + 0.03 * hiss;
                const double r = 0.45 * std::sin(2 * M_PI * 660.0 * s) - 0.2 * chirp + 0.03 * hiss;
                pcm[2 * n] = (INT_PCM)std::lrint(l * 32767.0);
                pcm[2 * n + 1] = (INT_PCM)std::lrint(r * 32767.0);
            }
            if (n == 0) flushing = true;
        }
        void *inPtr = pcm.data(), *outPtr = out.data();
        INT inId = IN_AUDIO_DATA, outId = OUT_BITSTREAM_DATA;
        INT inSize = n * 2 * (INT)sizeof(INT_PCM), inElem = (INT)sizeof(INT_PCM);
        INT outSize = (INT)out.size(), outElem = 1;
        AACENC_BufDesc inBuf = {}, outBuf = {};
        inBuf.numBufs = 1; inBuf.bufs = &inPtr; inBuf.bufferIdentifiers = &inId;
        inBuf.bufSizes = &inSize; inBuf.bufElSizes = &inElem;
        outBuf.numBufs = 1; outBuf.bufs = &outPtr; outBuf.bufferIdentifiers = &outId;
        outBuf.bufSizes = &outSize; outBuf.bufElSizes = &outElem;
        AACENC_InArgs inArgs = {};
        AACENC_OutArgs outArgs = {};
        inArgs.numInSamples = flushing ? -1 : n * 2;
        AACENC_ERROR err = aacEncEncode(enc, &inBuf, &outBuf, &inArgs, &outArgs);
        if (err == AACENC_ENCODE_EOF) break;
        if (err != AACENC_OK) { fprintf(stderr, "encode failed %d\n", err); return 1; }
        if (outArgs.numOutBytes > 0) {
            put16(f, (unsigned)outArgs.numOutBytes);
            fwrite(out.data(), 1, outArgs.numOutBytes, f);
            aus++;
        }
    }
    fclose(f);
    aacEncClose(&enc);
    fprintf(stderr, "%d access units\n", aus);
    return 0;
}
