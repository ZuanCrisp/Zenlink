// Cast player for Apple's screen stream (same protocol as PhoneLink V1 and
// upstream pane-iphone-mirror-windows): HEVC access units from
// /cast/stream.bin through WebCodecs, 48 kHz stereo PCM from /cast/audio.bin
// through WebAudio, and touch input posted back to /cast/touch.
//
// Picture quality rules (from upstream + V1):
// * the decoder is told the real stream size from the SPS, and a frame whose
//   visibleRect was clamped (WebView2 hardware HEVC -> 1280x720) is un-cropped,
//   so a 1648x2384 iPad stream is drawn with all of its pixels;
// * the canvas backing store is the stream's own size (1:1), CSS only scales;
// * the newest decoded frame is painted once per display refresh.
"use strict";

class CastPlayer {
  constructor(canvas, { onStats, onFirstFrame, onEnded, onResize, video } = {}) {
    this.canvas = canvas;
    this.video = video || null;
    this.videoMode = false;   // true: frames go straight to <video>
    this.lipSync = true;      // picture waits for the sound's output latency
    this._writer = null;
    // WebGL2: the decoded frame is uploaded as a GPU texture and drawn by a
    // shader (rotation + crop in texture coordinates). Canvas 2D drawImage of
    // a 2360x1640 frame took ~30 ms and painted only ~33 of 52 fps.
    // Not desynchronized: a low-latency canvas is a hardware overlay in
    // WebView2 and draws over the settings panel and full-screen bar.
    this.gl = this._initGL(canvas);
    this.ctx = this.gl ? null : canvas.getContext("2d", { alpha: false });
    this.onStats = onStats || (() => {});
    this.onFirstFrame = onFirstFrame || (() => {});
    this.onEnded = onEnded || (() => {});
    this.onResize = onResize || (() => {});
    this.generation = 0;
    this.decoder = null;
    this.reader = null;
    this.rotation = "auto";     // "auto" or a fixed 0/90/180/270
    this.deviceRotation = 0;    // from SpringBoard while casting
    this.appliedRotation = 0;   // what the last frame was drawn with
    this.volume = 1;
    this.muted = false;
    this.cushion = 0.06;
    this.audio = null;
    this.audioAbort = null;
    this.streamSize = { width: 0, height: 0 };
    this.frameSize = { width: 0, height: 0 };
    this.crop = { w: 0, h: 0 };
    this._pending = null;
    this._raf = 0;
    this._tick = 0;
    this.cropDetect = true;  // Apple/HEVC path only (see _paint)
    this._arrivals = new Map();
    this._det = document.createElement("canvas");
    this._detCtx = this._det.getContext("2d", { willReadFrequently: true });
    this._resetStats();
    this._diag = { errors: 0, lastError: "", rebuilds: 0, decoded: 0, painted: 0, maxQueue: 0, lat: 0, latN: 0 };
    this._diagAt = performance.now();
  }

  // Every 5 s, send what the player saw to the app log (zenlink.log).
  _reportDiag(now) {
    if (now - this._diagAt < 5000) return;
    const d = this._diag, secs = (now - this._diagAt) / 1000;
    const body = {
      codec: this.codecName, decodedFps: +(d.decoded / secs).toFixed(1), paintedFps: +(d.painted / secs).toFixed(1),
      decodeErrors: d.errors, rebuilds: d.rebuilds, maxDecodeQueue: d.maxQueue, coalesced: d.coalesced || 0,
      latencyMs: d.latN ? Math.round(d.lat / d.latN) : null, lastError: d.lastError,
      audioMs: this.audio ? Math.round(this._audioLat || 0) : null, audio: this._audioParts || null,
      hardware: this._hw, renderer: this.videoMode ? "video" : this.gl ? "webgl2" : "canvas2d",
    };
    fetch("/api/client-log", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) }).catch(() => {});
    this._diag = { errors: 0, lastError: "", rebuilds: 0, decoded: 0, painted: 0, maxQueue: 0, lat: 0, latN: 0 };
    this._diagAt = now;
  }

  // ---------------------------------------------------------------- GPU drawing

  _initGL(canvas) {
    let gl = null;
    try {
      gl = canvas.getContext("webgl2", {
        alpha: false, antialias: false, depth: false, stencil: false,
        premultipliedAlpha: false, preserveDrawingBuffer: false, powerPreference: "high-performance",
      });
    } catch (_) {}
    if (!gl) return null;
    const vs = `#version 300 es
      in vec2 p;
      out vec2 t;
      void main() { gl_Position = vec4(p, 0.0, 1.0); t = vec2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5); }`;
    const fs = `#version 300 es
      precision mediump float;
      in vec2 t;
      uniform sampler2D tex;
      uniform int rot;
      uniform vec2 crop;
      out vec4 o;
      void main() {
        vec2 s = t;                                   // screen (0,0) = top-left
        if (rot == 90) s = vec2(t.y, 1.0 - t.x);      // picture turned clockwise
        else if (rot == 180) s = vec2(1.0 - t.x, 1.0 - t.y);
        else if (rot == 270) s = vec2(1.0 - t.y, t.x);
        o = texture(tex, s * crop);
      }`;
    const compile = (type, src) => {
      const sh = gl.createShader(type);
      gl.shaderSource(sh, src);
      gl.compileShader(sh);
      if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(sh));
      return sh;
    };
    try {
      const prog = gl.createProgram();
      gl.attachShader(prog, compile(gl.VERTEX_SHADER, vs));
      gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, fs));
      gl.linkProgram(prog);
      if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
      gl.useProgram(prog);
      const buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
      const loc = gl.getAttribLocation(prog, "p");
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
      const tex = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
      this._glRot = gl.getUniformLocation(prog, "rot");
      this._glCrop = gl.getUniformLocation(prog, "crop");
      gl.uniform1i(gl.getUniformLocation(prog, "tex"), 0);
      return gl;
    } catch (error) {
      console.warn("WebGL renderer unavailable, using canvas 2D:", error);
      return null;
    }
  }

  _drawGL(src, cw, ch, sw, sh, w, h, rot) {
    const gl = this.gl;
    gl.viewport(0, 0, cw, ch);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, src);
    gl.uniform1i(this._glRot, rot);
    gl.uniform2f(this._glCrop, sw / w, sh / h);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }

  // ---------------------------------------------------------------- video

  async start() {
    const gen = ++this.generation;
    this._painted = false;
    const codec = await this._codec(gen);
    if (!codec || gen !== this.generation) return;

    this.codecName = codec.codec.startsWith("avc") ? "H.264" : "HEVC";
    this.canRequestKey = codec.keyframesOnRequest;
    this._reopen = false;
    // The grey-padding check reads pixels back from the GPU: only the Apple
    // (HEVC) path needs it, and only now and then.
    this.cropDetect = this.codecName === "HEVC" && this.canRequestKey;
    this.crop = { w: 0, h: 0 };
    this._openVideoOutput(true);
    const dims = codec.codec.startsWith("avc") ? null : parseHevcSpsDimensions(codec.description);
    this.streamSize = dims || { width: 0, height: 0 };
    const config = {
      codec: codec.codec, description: codec.description, optimizeForLatency: true,
      hardwareAcceleration: "prefer-hardware",
    };
    if (dims) {
      Object.assign(config, {
        codedWidth: dims.codedWidth,
        codedHeight: dims.codedHeight,
        displayAspectWidth: dims.width,
        displayAspectHeight: dims.height,
      });
    }
    let support = await VideoDecoder.isConfigSupported(config).catch(() => ({ supported: false }));
    if (!support.supported) {
      delete config.hardwareAcceleration;
      support = await VideoDecoder.isConfigSupported(config).catch(() => ({ supported: false }));
    }
    if (!support.supported && dims) {
      for (const k of ["codedWidth", "codedHeight", "displayAspectWidth", "displayAspectHeight"]) delete config[k];
      support = await VideoDecoder.isConfigSupported(config).catch(() => ({ supported: false }));
    }
    if (!support.supported) {
      if (gen === this.generation) {
        this.onEnded("This PC can't decode HEVC video. Install \"HEVC Video Extensions\" from the Microsoft Store.");
      }
      return;
    }

    let resp = null;
    for (let i = 0; i < 12 && gen === this.generation && !resp; i++) {
      if (i) await new Promise((r) => setTimeout(r, 400 + i * 250));
      try {
        const r = await fetch("/cast/stream.bin", { cache: "no-store" });
        if (r.ok) resp = r;
      } catch (_) {}
    }
    if (!resp || gen !== this.generation) return;
    this.reader = resp.body.getReader();
    this._buildDecoder(config);
    let buf = new Uint8Array(0);
    let ts = 0;
    let gotKey = false;
    let needsResync = false;
    this._needsResync = () => { needsResync = true; };
    try {
      while (gen === this.generation) {
        const { value, done } = await this.reader.read();
        if (done) break;
        const arrived = performance.now();
        const merged = new Uint8Array(buf.length + value.length);
        merged.set(buf);
        merged.set(value, buf.length);
        buf = merged;
        while (buf.length >= 5) {
          const len = (buf[0] << 24) | (buf[1] << 16) | (buf[2] << 8) | buf[3];
          if (buf.length < 4 + len) break;
          const type = buf[4];
          const data = buf.slice(5, 4 + len);
          buf = buf.subarray(4 + len);
          if (type === 3) {
            // New codec configuration (USB Direct: rotation / size change).
            try {
              const info = JSON.parse(new TextDecoder().decode(data));
              config.codec = info.codec.trim();
              config.description = Uint8Array.from(atob(info.description), (c) => c.charCodeAt(0));
              this.codecName = config.codec.startsWith("avc") ? "H.264" : "HEVC";
              for (const k of ["codedWidth", "codedHeight", "displayAspectWidth", "displayAspectHeight"]) delete config[k];
              this.streamSize = { width: 0, height: 0 };
              this._buildDecoder(config);
              gotKey = false;
            } catch (_) {}
            continue;
          }
          const isKey = type === 0 || type === 2;
          if (type === 2 || (isKey && needsResync) || this.decoder.state !== "configured") {
            this._buildDecoder(config);
            needsResync = false;
          }
          if (isKey) gotKey = true;
          if (!gotKey || needsResync) continue;
          if (this.canRequestKey && !isKey && this.codecName === "HEVC" && this.decoder.decodeQueueSize > 4) {
            // Apple mode: the decoder is behind, so skip to a fresh keyframe
            // (requested by PLI) instead of building up delay. USB Direct
            // can't request keyframes, so it never skips.
            needsResync = true;
            fetch("/cast/pli", { method: "POST" }).catch(() => {});
            continue;
          }
          ts += 16667;
          this._stats.bytes += data.length;
          this._arrivals.set(ts, arrived);
          if (this._arrivals.size > 240) this._arrivals.clear();
          try {
            this.decoder.decode(new EncodedVideoChunk({ type: isKey ? "key" : "delta", timestamp: ts, data }));
          } catch (_) {
            if (this.canRequestKey) needsResync = true;
            else this._reopenStream();
          }
        }
      }
    } catch (_) {
      /* stream closed */
    }
    if (gen === this.generation && !this.canRequestKey && !this._reopen) {
      // AirPlay: the server ended this stream because the window fell too far
      // behind; reconnect (it replays from the last keyframe) instead of stopping.
      this._reopen = true;
    }
    if (gen === this.generation && this._reopen) {
      // No keyframe on request (AirPlay): reconnect; the server replays the
      // stream from its last keyframe, so the decoder rebuilds the picture.
      // At most once every 3 s, so a persistent problem can't spin.
      this._reopen = false;
      this._diag.rebuilds++;
      const wait = Math.max(0, 3000 - (performance.now() - (this._lastReopen || 0)));
      this._lastReopen = performance.now() + wait;
      if (wait) await new Promise((r) => setTimeout(r, wait));
      if (gen === this.generation) return this.start();
      return;
    }
    if (gen === this.generation) this.onEnded(null);
  }

  _reopenStream() {
    if (this._reopen) return;
    this._reopen = true;
    try { this.reader && this.reader.cancel(); } catch (_) {}
  }

  async _codec(gen) {
    for (let i = 0; i < 20 && gen === this.generation; i++) {
      try {
        const r = await fetch("/cast/codec", { cache: "no-store" });
        if (r.ok) {
          const info = await r.json();
          if (info.codec && info.description) {
            return {
              codec: info.codec.trim(),
              description: Uint8Array.from(atob(info.description), (c) => c.charCodeAt(0)),
              // Apple mode asks the device for a fresh keyframe (PLI); AirPlay and
              // ZenLink Studio can't, so they must never wait for one.
              keyframesOnRequest: info.keyframesOnRequest !== false,
            };
          }
        }
      } catch (_) {}
      await new Promise((r) => setTimeout(r, 300 + i * 200));
    }
    return null;
  }

  _buildDecoder(config) {
    this._diag.rebuilds++;
    this._hw = config.hardwareAcceleration || "no-preference";
    if (this.decoder) {
      try { this.decoder.close(); } catch (_) {}
    }
    this._arrivals = new Map();
    this.decoder = new VideoDecoder({
      output: (frame) => this._queue(frame),
      error: (e) => {
        this._diag.errors++;
        this._diag.lastError = String(e && e.message || e);
        if (!this.canRequestKey) { this._reopenStream(); return; }
        // Resync on a fresh keyframe from the device.
        if (this._needsResync) this._needsResync();
        fetch("/cast/pli", { method: "POST" }).catch(() => {});
      },
    });
    this.decoder.configure(config);
  }

  // Both modes hand decoded frames to a MediaStreamTrack that a <video> element
  // plays, so the browser presents them like any video, in sync with the
  // display, instead of ZenLink drawing each one (as iPhoneMirror does).
  // Crop / un-crop only rewrap the frame's visible rectangle (no pixel copy);
  // rotation is a CSS transform on the <video>.
  _openVideoOutput(want) {
    this._closeVideoOutput();
    if (!want || !this.video || typeof MediaStreamTrackGenerator === "undefined") return;
    try {
      const track = new MediaStreamTrackGenerator({ kind: "video" });
      this._writer = track.writable.getWriter();
      this.video.srcObject = new MediaStream([track]);
      this.video.play().catch(() => {});
      this.videoMode = true;
      this._videoRot = -1;
      if (!this._stageObserver && this.video.parentElement) {
        this._stageObserver = new ResizeObserver(() => this._layoutVideo(true));
        this._stageObserver.observe(this.video.parentElement);
      }
    } catch (error) {
      console.warn("Direct video output unavailable:", error);
      this._closeVideoOutput();
    }
  }

  _closeVideoOutput() {
    this._dropDue();
    this.videoMode = false;
    const writer = this._writer;
    this._writer = null;
    if (writer) { try { writer.close(); } catch (_) {} }
    if (this.video) {
      try { this.video.srcObject = null; } catch (_) {}
    }
  }

  // Turn the <video> for the device's orientation: a 90/270 picture gets a box
  // with swapped sides, centred in the stage, then rotated.
  _layoutVideo(force = false) {
    const v = this.video, stage = v && v.parentElement;
    if (!v || !stage) return;
    const rot = this.appliedRotation || 0;
    if (!force && rot === this._videoRot) return;
    this._videoRot = rot;
    if (rot === 90 || rot === 270) {
      const W = stage.clientWidth, H = stage.clientHeight;
      Object.assign(v.style, {
        width: `${H}px`, height: `${W}px`, left: `${(W - H) / 2}px`, top: `${(H - W) / 2}px`,
        right: "auto", bottom: "auto", transform: `rotate(${rot}deg)`,
      });
    } else {
      Object.assign(v.style, { width: "", height: "", left: "", top: "", right: "", bottom: "",
        transform: rot === 180 ? "rotate(180deg)" : "" });
    }
  }

  _present(frame) {
    // Full picture back if the decoder clamped it, then stretch a motion-shrunk
    // capture back to the full frame: both are visibleRect rewraps.
    const fixed = this._uncrop(frame);
    let src = fixed || frame;
    let w = src.displayWidth, h = src.displayHeight;
    const ss = this.streamSize;
    if (ss.width && ss.width <= w && ss.height <= h && w - ss.width < 64 && h - ss.height < 64) { w = ss.width; h = ss.height; }
    if (this.cropDetect && this._tick++ % 6 === 0) this._detectCrop(src);
    const sw = this.crop.w || w, sh = this.crop.h || h;
    let out = src;
    if (sw !== src.displayWidth || sh !== src.displayHeight || w !== src.displayWidth || h !== src.displayHeight) {
      try {
        out = new VideoFrame(src, { visibleRect: { x: 0, y: 0, width: sw, height: sh }, displayWidth: w, displayHeight: h });
      } catch (_) {
        out = src;
      }
    }
    const rot = this.effectiveRotation(w, h);
    this.appliedRotation = rot;
    this._layoutVideo();
    if (this.frameSize.width !== w || this.frameSize.height !== h) {
      this.frameSize = { width: w, height: h };
      const turned = rot === 90 || rot === 270;
      this.onResize({ width: turned ? h : w, height: turned ? w : h });
    }
    const arrived = this._arrivals.get(frame.timestamp);
    this._arrivals.delete(frame.timestamp);
    // Never queue up: if the track is several frames behind, drop this one.
    const keep = out !== frame ? out : frame;
    if (this._writer.desiredSize !== null && this._writer.desiredSize < -2) {
      keep.close();
    } else {
      const delay = this.lipSync && (this.audio || this.nativeAudio) && this._audioLat > 0 ? Math.min(250, this._audioLat) : 0;
      this._schedule(keep, performance.now() + delay);
    }
    if (out !== frame) frame.close();
    if (fixed && fixed !== out) fixed.close();
    if (!this._painted) {
      this._painted = true;
      this.onFirstFrame();
    }
    const s = this._stats, now = performance.now();
    s.frames++;
    if (arrived) { s.lat += now - arrived; s.n++; this._diag.lat += now - arrived; this._diag.latN++; }
    this._diag.painted++;
    this._reportDiag(now);
    if (now - s.t0 >= 1000) {
      const secs = (now - s.t0) / 1000;
      this.onStats({
        width: w, height: h, fps: Math.round(s.frames / secs),
        latency: s.n ? Math.round(s.lat / s.n) : null, audio: s.audio > 0, audioMs: s.audio > 0 ? Math.round(this._audioLat || 0) : null,
        mbps: Math.round((s.bytes * 8) / secs / 1e5) / 10,
      });
      this._resetStats(now);
    }
  }

  // Frames reach the <video> track on the display's refresh (requestAnimationFrame),
  // not whenever the decoder or a lip-sync timer happens to fire. Writes that
  // land at random points between refreshes make the picture look smooth in
  // ZenLink's own window but uneven in OBS, which samples the composed desktop
  // on its own clock. When two frames fall due within one refresh only the
  // newer one is shown (the older could never have been seen anyway).
  // A timer backs rAF up, because Chromium pauses rAF for windows it considers
  // hidden (for example fully covered) while OBS may still be capturing them.
  _schedule(frame, at) {
    const q = this._due || (this._due = []);
    q.push({ frame, at, writer: this._writer });
    while (q.length > 6) { try { q.shift().frame.close(); } catch (_) {} }
    if (!this._dueRaf) this._dueRaf = requestAnimationFrame(() => this._flushDue("raf"));
    if (!this._dueTimer) this._dueTimer = setTimeout(() => this._flushDue("timer"), 34);
  }

  _flushDue(source) {
    if (source === "raf") this._dueRaf = 0;
    else this._dueTimer = 0;
    const q = this._due || [];
    const t = performance.now();
    let ready = null;
    while (q.length && q[0].at <= t + 4) {
      if (ready) { try { ready.frame.close(); } catch (_) {} this._diag.coalesced = (this._diag.coalesced || 0) + 1; }
      ready = q.shift();
    }
    if (ready) {
      if (this._writer === ready.writer && this._writer) ready.writer.write(ready.frame).catch(() => {});
      else { try { ready.frame.close(); } catch (_) {} }
    }
    if (q.length) {
      if (!this._dueRaf) this._dueRaf = requestAnimationFrame(() => this._flushDue("raf"));
      if (!this._dueTimer) this._dueTimer = setTimeout(() => this._flushDue("timer"), 34);
    } else if (this._dueTimer && source === "raf") {
      clearTimeout(this._dueTimer); this._dueTimer = 0;
    }
  }

  _dropDue() {
    for (const d of this._due || []) { try { d.frame.close(); } catch (_) {} }
    this._due = [];
    if (this._dueRaf) { cancelAnimationFrame(this._dueRaf); this._dueRaf = 0; }
    if (this._dueTimer) { clearTimeout(this._dueTimer); this._dueTimer = 0; }
  }

  // Newest frame wins; paint once per display refresh.
  _queue(frame) {
    this._diag.decoded++;
    if (this.videoMode && this._writer) {
      this._present(frame);
      return;
    }
    if (this.decoder) this._diag.maxQueue = Math.max(this._diag.maxQueue, this.decoder.decodeQueueSize);
    if (this._pending) {
      try { this._pending.close(); } catch (_) {}
    }
    this._pending = frame;
    if (!this._raf) this._raf = requestAnimationFrame(() => this._drawPending());
  }

  _drawPending() {
    this._raf = 0;
    const frame = this._pending;
    this._pending = null;
    if (frame) this._paint(frame);
  }

  // A frame whose visibleRect the decoder clamped below the coded size gets
  // its full picture back (V1).
  _uncrop(frame) {
    const vr = frame.visibleRect;
    if (!frame.codedWidth || !frame.codedHeight || !vr) return null;
    const want = this.streamSize;
    const w = want.width && want.width <= frame.codedWidth ? want.width : frame.codedWidth;
    const h = want.height && want.height <= frame.codedHeight ? want.height : frame.codedHeight;
    if (vr.width >= w && vr.height >= h && frame.displayWidth >= w && frame.displayHeight >= h) return null;
    try {
      return new VideoFrame(frame, {
        visibleRect: { x: 0, y: 0, width: w, height: h },
        displayWidth: w,
        displayHeight: h,
      });
    } catch (_) {
      return null;
    }
  }

  // Under fast motion the device encoder can capture at a smaller size and
  // pad the fixed frame with flat grey; stretch the real content back (V1).
  _detectCrop(frame) {
    const fw = frame.displayWidth, fh = frame.displayHeight;
    const H = 160, W = Math.max(8, Math.round((H * fw) / fh));
    if (this._det.width !== W) this._det.width = W;
    if (this._det.height !== H) this._det.height = H;
    this.crop = { w: 0, h: 0 };
    try {
      this._detCtx.drawImage(frame, 0, 0, W, H);
      const d = this._detCtx.getImageData(0, 0, W, H).data;
      const grey = (x, y) => {
        const i = (y * W + x) * 4;
        return Math.abs(d[i] - 128) < 6 && Math.abs(d[i + 1] - 128) < 6 && Math.abs(d[i + 2] - 128) < 6;
      };
      let cr = 0, cb = 0;
      for (let x = 0; x < W; x++) { let g = 0, n = 0; for (let y = 0; y < H; y += 4) { n++; if (grey(x, y)) g++; } if (g / n < 0.6) cr = x; }
      for (let y = 0; y < H; y++) { let g = 0, n = 0; for (let x = 0; x < W; x += 4) { n++; if (grey(x, y)) g++; } if (g / n < 0.6) cb = y; }
      const cw = Math.round((cr / W) * fw), ch = Math.round((cb / H) * fh);
      if (cw > fw * 0.2 && cw < fw * 0.92 && ch > fh * 0.2 && ch < fh * 0.92) this.crop = { w: cw, h: ch };
    } catch (_) {}
  }

  // Rotation the picture is drawn with. Auto follows the device; when iOS
  // already delivers a landscape buffer the picture is upright as-is.
  effectiveRotation(w, h) {
    if (this.rotation !== "auto") return this.rotation;
    if (w > h) return 0;
    return this.deviceRotation || 0;
  }

  _paint(frame) {
    const fixed = this._uncrop(frame);
    const src = fixed || frame;
    let w = src.displayWidth, h = src.displayHeight;
    // Never draw the encoder's padding rows/columns.
    const ss = this.streamSize;
    if (ss.width && ss.width <= w && ss.height <= h && w - ss.width < 64 && h - ss.height < 64) { w = ss.width; h = ss.height; }
    if (this.cropDetect && this._tick++ % 6 === 0) this._detectCrop(src);
    const rot = this.effectiveRotation(w, h);
    this.appliedRotation = rot;
    const turned = rot === 90 || rot === 270;
    const cw = turned ? h : w, ch = turned ? w : h;
    if (this.canvas.width !== cw || this.canvas.height !== ch) {
      this.canvas.width = cw;
      this.canvas.height = ch;
      this.onResize({ width: cw, height: ch });
    }
    this.frameSize = { width: w, height: h };
    const sw = this.crop.w || w, sh = this.crop.h || h;
    if (this.gl) {
      this._drawGL(src, cw, ch, sw, sh, w, h, rot);
    } else {
      const ctx = this.ctx;
      // Canvas and frame have the same size: no resampling needed here.
      ctx.imageSmoothingEnabled = false;
      ctx.save();
      ctx.translate(cw / 2, ch / 2);
      if (rot) ctx.rotate((rot * Math.PI) / 180);
      ctx.drawImage(src, 0, 0, sw, sh, -w / 2, -h / 2, w, h);
      ctx.restore();
    }
    const arrived = this._arrivals.get(frame.timestamp);
    this._arrivals.delete(frame.timestamp);
    if (fixed) fixed.close();
    frame.close();
    if (!this._painted) {
      this._painted = true;
      this.onFirstFrame();
    }
    const s = this._stats, now = performance.now();
    s.frames++;
    if (arrived) { s.lat += now - arrived; s.n++; this._diag.lat += now - arrived; this._diag.latN++; }
    this._diag.painted++;
    this._reportDiag(now);
    if (now - s.t0 >= 1000) {
      const secs = (now - s.t0) / 1000;
      this.onStats({
        width: w, height: h,
        fps: Math.round(s.frames / secs),
        latency: s.n ? Math.round(s.lat / s.n) : null,
        audio: s.audio > 0, audioMs: s.audio > 0 ? Math.round(this._audioLat || 0) : null,
        mbps: Math.round((s.bytes * 8) / secs / 1e5) / 10,
      });
      this._resetStats(now);
    }
  }

  _resetStats(now = performance.now()) {
    this._stats = { frames: 0, lat: 0, n: 0, audio: 0, bytes: 0, t0: now };
  }

  stop() {
    this.generation++;
    try { this.reader?.cancel(); } catch (_) {}
    this.reader = null;
    if (this.decoder) {
      try { this.decoder.close(); } catch (_) {}
    }
    this.decoder = null;
    if (this._raf) cancelAnimationFrame(this._raf);
    this._raf = 0;
    try { this._pending?.close(); } catch (_) {}
    this._pending = null;
    this.crop = { w: 0, h: 0 };
    this._closeVideoOutput();
    this.stopAudio();
  }

  // ---------------------------------------------------------------- audio

  async startAudio() {
    if (this.audioAbort) return;
    const abort = new AbortController();
    this.audioAbort = abort;
    this.audio = new AudioContext({ sampleRate: 48000, latencyHint: 0 });  // smallest output buffer Windows allows
    this.gain = this.audio.createGain();
    this.gain.gain.value = this.muted ? 0 : this.volume;
    this.gain.connect(this.audio.destination);
    // Ring-buffer worklet: plays samples as they arrive (see audio-worklet.js).
    this._pcmNode = null;
    try {
      await this.audio.audioWorklet.addModule(`/ui/audio-worklet.js?v=${Date.now()}`);
      this._pcmNode = new AudioWorkletNode(this.audio, "zenlink-pcm", { outputChannelCount: [2] });
      this._pcmNode.connect(this.gain);
      this._pcmNode.port.onmessage = (e) => { this._audioQueue = e.data; };
    } catch (error) {
      this._diag.lastError = `audio worklet: ${error}`;
      console.warn("Audio worklet unavailable, scheduling buffers instead:", error);
    }
    let next = 0;
    try {
      const resp = await fetch("/cast/audio.bin", { cache: "no-store", signal: abort.signal });
      if (!resp.ok) throw new Error(`audio ${resp.status}`);
      const reader = resp.body.getReader();
      let buf = new Uint8Array(0);
      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        const merged = new Uint8Array(buf.length + value.length);
        merged.set(buf);
        merged.set(value, buf.length);
        buf = merged;
        while (buf.length >= 4) {
          const len = (buf[0] << 24) | (buf[1] << 16) | (buf[2] << 8) | buf[3];
          if (buf.length < 4 + len) break;
          const pcm = buf.slice(4, 4 + len);
          buf = buf.subarray(4 + len);
          next = this._playPcm(pcm, next);
        }
      }
    } catch (error) {
      if (error?.name !== "AbortError") this._diag.lastError = `audio: ${error}`;
    }
    if (this.audioAbort === abort) {
      this.audioAbort = null;
      this.audio?.close().catch(() => {});
      this.audio = null;
    }
  }

  _playPcm(bytes, next) {
    const ctx = this.audio;
    if (!ctx || !bytes.length) return next;
    if (ctx.state === "suspended") ctx.resume().catch(() => {});
    this._stats.audio++;
    if (this._pcmNode) {
      const s16 = new Int16Array(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + (bytes.byteLength & ~1)));
      this._pcmNode.port.postMessage(s16, [s16.buffer]);
      const q = this._audioQueue || { queueMs: 0 };
      const outMs = ((ctx.baseLatency || 0) + (ctx.outputLatency || 0)) * 1000;
      this._audioLat = q.queueMs + outMs;
      this._audioParts = { queueMs: Math.round(q.queueMs), outputMs: Math.round(outMs),
        targetMs: Math.round(q.targetMs || 0), underruns: q.underruns || 0, droppedMs: Math.round(q.droppedMs || 0) };
      return next;
    }
    const s16 = new Int16Array(bytes.buffer, bytes.byteOffset, bytes.byteLength >> 1);
    const frames = s16.length >> 1;
    const buf = ctx.createBuffer(2, frames, 48000);
    const l = buf.getChannelData(0), r = buf.getChannelData(1);
    for (let i = 0, j = 0; i < frames; i++, j += 2) { l[i] = s16[j] / 32768; r[i] = s16[j + 1] / 32768; }
    const src = ctx.createBufferSource();
    src.buffer = buf;
    src.connect(this.gain);
    const now = ctx.currentTime;
    // Keep the queue between 5 ms and 2x the cushion: anything older is
    // dropped so audio never drifts behind the picture.
    if (next < now + 0.005 || next > now + this.cushion * 2) next = now + this.cushion;
    src.start(next);
    // Audio latency = what is queued ahead + Windows' own output path.
    const out = (ctx.baseLatency || 0) + (ctx.outputLatency || 0);
    this._audioLat = (this._audioLat || 0) * 0.9 + ((next - now) + out) * 1000 * 0.1;
    return next + buf.duration;
  }

  stopAudio() {
    const abort = this.audioAbort;
    this.audioAbort = null;
    try { abort?.abort(); } catch (_) {}
    try { this._pcmNode?.disconnect(); } catch (_) {}
    this._pcmNode = null;
    this.audio?.close().catch(() => {});
    this.audio = null;
  }

  setRotation(value) {
    const deg = Number(value);
    this.rotation = value === "auto" || ![0, 90, 180, 270].includes(deg) ? "auto" : deg;
  }
  setDeviceRotation(deg) { this.deviceRotation = [0, 90, 180, 270].includes(deg) ? deg : 0; }
  setAudioBuffer(level) { this.cushion = { low: 0.025, normal: 0.06, high: 0.15 }[level] || 0.06; }
  setVolume(v) {
    this.volume = v;
    if (this.gain && !this.muted) this.gain.gain.setTargetAtTime(v, this.audio.currentTime, 0.02);
  }
  setMuted(m) {
    this.muted = m;
    if (this.gain) this.gain.gain.setTargetAtTime(m ? 0 : this.volume, this.audio.currentTime, 0.02);
  }
  reset() { this.stop(); }

  // ---------------------------------------------------------------- input

  // Map a pointer position on the canvas to the device's 0..65535 touch
  // space, undoing object-fit (contain letterboxing or cover cropping) and
  // the rotation the picture is drawn with. With clamp, points off the
  // picture land on its edge.
  touchPoint(clientX, clientY, clamp = false) {
    const surface = this.videoMode && this.video ? this.video.parentElement : this.canvas;
    const rect = surface.getBoundingClientRect();
    const rot0 = this.appliedRotation;
    const turned0 = rot0 === 90 || rot0 === 270;
    const cw = this.videoMode ? (turned0 ? this.frameSize.height : this.frameSize.width) : this.canvas.width;
    const ch = this.videoMode ? (turned0 ? this.frameSize.width : this.frameSize.height) : this.canvas.height;
    if (!cw || !ch || !rect.width || !rect.height) return null;
    const fit = getComputedStyle(this.videoMode && this.video ? this.video : this.canvas).objectFit;
    const sx = rect.width / cw, sy = rect.height / ch;
    const scale = fit === "cover" ? Math.max(sx, sy) : fit === "fill" ? 0 : Math.min(sx, sy);
    const dw = scale ? cw * scale : rect.width, dh = scale ? ch * scale : rect.height;
    const ox = rect.left + (rect.width - dw) / 2, oy = rect.top + (rect.height - dh) / 2;
    let u = (clientX - ox) / dw, v = (clientY - oy) / dh;
    if (clamp) { u = Math.min(1, Math.max(0, u)); v = Math.min(1, Math.max(0, v)); }
    else if (u < 0 || v < 0 || u > 1 || v > 1) return null;
    const rot = this.appliedRotation;
    if (rot === 90) [u, v] = [v, 1 - u];
    else if (rot === 180) [u, v] = [1 - u, 1 - v];
    else if (rot === 270) [u, v] = [1 - v, u];
    return { x: Math.round(u * 65535), y: Math.round(v * 65535) };
  }
}

// Width/height from the HEVC SPS inside the hvcC record (PhoneLink V1).
function parseHevcSpsDimensions(description) {
  try {
    const bytes = description instanceof Uint8Array ? description : new Uint8Array(description);
    if (!bytes || bytes.length < 23) return null;
    let sps = null;
    if (bytes[0] === 1) {
      let off = 23;
      for (let i = 0; i < bytes[22] && off + 3 <= bytes.length && !sps; i++) {
        const type = bytes[off] & 0x3f;
        const count = (bytes[off + 1] << 8) | bytes[off + 2];
        off += 3;
        for (let j = 0; j < count && off + 2 <= bytes.length; j++) {
          const len = (bytes[off] << 8) | bytes[off + 1];
          off += 2;
          if (type === 33 && !sps) sps = bytes.subarray(off, off + len);
          off += len;
        }
      }
    } else {
      sps = bytes;
    }
    if (!sps || sps.length < 15) return null;
    // Drop emulation-prevention bytes (00 00 03 -> 00 00), skip the NAL header.
    const rb = [];
    for (let i = 2; i < sps.length; i++) {
      if (i + 2 < sps.length && sps[i] === 0 && sps[i + 1] === 0 && sps[i + 2] === 3) { rb.push(0, 0); i += 2; }
      else rb.push(sps[i]);
    }
    let pos = 0;
    const bit = () => { const b = (rb[pos >> 3] >> (7 - (pos & 7))) & 1; pos++; return b; };
    const bits = (n) => { let v = 0; for (let k = 0; k < n; k++) v = v * 2 + bit(); return v; };
    const ue = () => {
      let z = 0;
      while (bit() === 0 && pos < rb.length * 8) z++;
      return z ? 2 ** z + bits(z) - 1 : 0;
    };
    bits(4);                       // sps_video_parameter_set_id
    const maxSub = bits(3);
    bits(1);                       // temporal_id_nesting
    bits(2 + 1 + 5 + 32 + 48 + 8); // general profile_tier_level
    const subP = [], subL = [];
    for (let i = 0; i < maxSub; i++) { subP.push(bit()); subL.push(bit()); }
    if (maxSub > 0) for (let i = maxSub; i < 8; i++) bits(2);
    for (let i = 0; i < maxSub; i++) { if (subP[i]) bits(88); if (subL[i]) bits(8); }
    ue();                          // sps_seq_parameter_set_id
    const chroma = ue();
    if (chroma === 3) bit();       // separate_colour_plane
    let width = ue(), height = ue();
    const codedWidth = width, codedHeight = height;
    if (bit()) {                   // conformance window: encoder padding to crop away
      const subW = chroma === 1 || chroma === 2 ? 2 : 1, subH = chroma === 1 ? 2 : 1;
      const l = ue(), r = ue(), t = ue(), b = ue();
      width -= subW * (l + r);
      height -= subH * (t + b);
    }
    return width > 0 && height > 0 ? { width, height, codedWidth, codedHeight } : null;
  } catch (_) {
    return null;
  }
}

window.CastPlayer = CastPlayer;
window.parseHevcSpsDimensions = parseHevcSpsDimensions;
