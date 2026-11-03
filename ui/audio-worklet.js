// Low-latency PCM sink: a small ring buffer read by the audio thread.
// Samples play as soon as they arrive; the fill target adapts to the size of
// the packets the device sends (enough to bridge one packet of jitter), and
// anything above the ceiling is dropped so sound never trails the picture.
"use strict";

class ZenLinkPcm extends AudioWorkletProcessor {
  constructor() {
    super();
    this.size = 48000;                // 1 s of stereo frames
    this.l = new Float32Array(this.size);
    this.r = new Float32Array(this.size);
    this.read = 0;
    this.fill = 0;                    // frames queued
    this.target = 240;                // 5 ms to start
    this.started = false;
    this.packet = 0;                  // largest recent packet, in frames
    this.underruns = 0;
    this.dropped = 0;
    this.margin = 128;                // extra frames kept against jitter
    this.calmSince = 0;
    this.reportAt = 0;
    this.port.onmessage = (e) => this.push(e.data);
  }

  push(s16) {
    const frames = s16.length >> 1;
    this.packet = Math.max(frames, this.packet * 0.995);
    // Bridge one packet of jitter plus an adaptive margin, nothing more.
    this.target = Math.min(4800, Math.ceil(this.packet + this.margin));
    const ceiling = this.target * 2 + 128;
    if (this.fill + frames > ceiling) {
      const drop = this.fill + frames - this.target;
      const skip = Math.min(drop, this.fill);
      this.read = (this.read + skip) % this.size;
      this.fill -= skip;
      this.dropped += skip;
    }
    let w = (this.read + this.fill) % this.size;
    for (let i = 0, j = 0; i < frames; i++, j += 2) {
      this.l[w] = s16[j] / 32768;
      this.r[w] = s16[j + 1] / 32768;
      w = (w + 1) % this.size;
    }
    this.fill = Math.min(this.size, this.fill + frames);
  }

  process(_inputs, outputs) {
    const out = outputs[0];
    const L = out[0], R = out[1] || out[0];
    const n = L.length;
    if (!this.started && this.fill >= this.target) this.started = true;
    if (this.started && this.fill >= n) {
      for (let i = 0; i < n; i++) {
        L[i] = this.l[this.read];
        R[i] = this.r[this.read];
        this.read = (this.read + 1) % this.size;
      }
      this.fill -= n;
    } else {
      L.fill(0);
      R.fill(0);
      if (this.started) {
        this.underruns++;
        this.started = false;
        this.margin = Math.min(1440, this.margin + 96);   // +2 ms, up to 30 ms
        this.calmSince = currentTime;
      }
    }
    if (currentTime - this.calmSince > 5 && this.margin > 128) {
      this.margin -= 48;                                   // -1 ms after 5 s calm
      this.calmSince = currentTime;
    }
    if (currentTime - this.reportAt >= 0.5) {
      this.reportAt = currentTime;
      this.port.postMessage({ queueMs: (this.fill / sampleRate) * 1000, targetMs: (this.target / sampleRate) * 1000,
        underruns: this.underruns, droppedMs: (this.dropped / sampleRate) * 1000 });
    }
    return true;
  }
}

registerProcessor("zenlink-pcm", ZenLinkPcm);
