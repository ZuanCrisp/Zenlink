"use strict";

const $ = (sel) => document.querySelector(sel);

const els = {
  stage: $("#stage"),
  screen: $("#screen"),
  idle: $("#idle"),
  deviceArt: $("#device-art"),
  deviceChip: $("#device-chip"),
  deviceChipName: $("#device-chip-name"),
  deviceChipStatus: $("#device-chip-status"),
  renameBtn: $("#device-rename"),
  renameInput: $("#device-rename-input"),
  idleTitle: $("#idle-title"),
  idleSub: $("#idle-sub"),
  connect: $("#connect"),
  connectTitle: $("#connect-title"),
  connectPct: $("#connect-pct"),
  progressBar: $("#progress-bar"),
  flow: [...document.querySelectorAll("#flow li")],
  connectError: $("#connect-error"),
  connectErrorText: $("#connect-error-text"),
  retryBtn: $("#btn-retry"),
  tally: $("#tally"),
  tallyText: $("#tally-text"),
  castBtn: $("#btn-cast"),
  muteBtn: $("#btn-mute"),
  volume: $("#volume"),
  themeBtn: $("#btn-theme"),
  fullscreenBtn: $("#btn-fullscreen"),
  adminBanner: $("#admin-banner"),
  metrics: { device: $("#m-device"), res: $("#m-res"), fps: $("#m-fps"), rate: $("#m-rate"), lat: $("#m-lat"), audio: $("#m-audio") },
  toasts: $("#toasts"),
};

const prefs = {
  get(key, fallback) {
    try {
      let v = localStorage.getItem(`zenlink.${key}`);
      if (v === null) v = localStorage.getItem(`phonelink.${key}`);  // settings from before the rename
      return v === null ? fallback : JSON.parse(v);
    } catch (_) {
      return fallback;
    }
  },
  set(key, value) {
    try { localStorage.setItem(`zenlink.${key}`, JSON.stringify(value)); } catch (_) {}
  },
};

let state = { phase: "idle", device: null, devices: [] };
let flowStep = -1;        // index of the furthest step reached in this attempt
let showingConnect = false;
let revealTimer = null;

const player = new CastPlayer(els.screen, {
  video: $("#screen-video"),
  onStats: renderMetrics,
  onFirstFrame: () => {
    advanceFlow(4);
    els.tally.dataset.state = "live";
    els.tallyText.textContent = "Live";
  },
  onEnded: (message) => { if (message) toast(message, "bad"); },
  onResize: (size) => {
    prefs.set("nativeSize", player.frameSize.width ? player.frameSize : size);
    sizeScreen();
    if (settings.fitWindow) window.ZenLinkWindow?.fitAspect(size.width, size.height);
  },
});
player.setVolume(prefs.get("volume", 100) / 100);
player.setMuted(prefs.get("muted", false));
els.volume.value = String(prefs.get("volume", 100));
els.volume.style.setProperty("--fill", `${els.volume.value}%`);
els.muteBtn.setAttribute("aria-pressed", String(prefs.get("muted", false)));

// ---------------------------------------------------------------- server

async function api(path, body) {
  const resp = await fetch(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body || {}),
  });
  return resp.json();
}

function connect() {
  const ws = new WebSocket(`ws://${location.host}/ws`);
  ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    if (msg.type === "state") applyState(msg);
  };
  ws.onclose = () => setTimeout(connect, 1000);
}

// ---------------------------------------------------------------- state

const CASTING = ["connecting", "activating", "streaming"];
// Checklist step each session phase is working on; the first painted frame
// completes the last step.
const PHASE_STEP = { connecting: 1, activating: 2, streaming: 3 };
const TALLY = {
  idle: ["idle", "Off air"],
  connecting: ["wait", "Connecting"],
  activating: ["wait", "Preparing"],
  streaming: ["wait", "Starting"],
  error: ["error", "Failed"],
};

function applyState(next) {
  const prevPhase = state.phase;
  state = next;
  const phase = state.phase;
  const casting = CASTING.includes(phase);
  const device = (state.devices || [])[0] || null;

  const [tone, label] = TALLY[phase] || TALLY.idle;
  els.tally.dataset.state = tone;
  els.tallyText.textContent = label;

  els.adminBanner.hidden = !state.takenOver;
  els.castBtn.textContent = casting ? "Stop" : "Start casting";
  els.castBtn.className = casting ? "btn stop" : "btn primary";
  els.castBtn.disabled = !casting && !(device && device.ready);

  renderIdle(device);
  renderConnection();
  const fsDev = $("#fs-device");
  if (fsDev) fsDev.textContent = state.device?.name || device?.name || "ZenLink";
  setTimeout(maybeAutoStart, 0);
  els.metrics.device.textContent = state.device?.name || device?.name || "—";
  renderHeaderInfo(state.phase === "streaming" ? lastStats : null);

  player.setDeviceRotation(state.rotation);
  player.nativeAudio = Boolean(state.nativeAudio);
  if (phase === "streaming" && prevPhase !== "streaming") {
    player.start();
    // Native sound (server side, ~22 ms) when available, else browser audio.
    if (settings.audioOut === "pc" && !state.nativeAudio) player.startAudio();
    pushAudioSettings();
  }
  if (phase === "streaming" && player._painted) {
    els.tally.dataset.state = "live";
    els.tallyText.textContent = "Live";
  }
  // AirPlay: the device comes to us, so while waiting keep the how-to visible
  // instead of the cable checklist.
  const airplayWaiting = Boolean(device?.airplay) && casting && phase !== "streaming";
  if (airplayWaiting) {
    closeConnect();
  } else if (casting) {
    if (!showingConnect && !player._painted) openConnect(state.device?.name || device?.name);
    if (showingConnect) advanceFlow(PHASE_STEP[phase]);
  } else if (phase === "error" && showingConnect) {
    failFlow(state.error || "Casting stopped.");
  } else if (phase === "error" && state.error && prevPhase !== "error") {
    toast(state.error, "bad");
  }

  if (!casting && phase !== "error") closeConnect();
  if (prevPhase === "streaming" && phase !== "streaming") {
    player.stop();
    renderMetrics(null);
    showScreen(false);
  }
}

function renderIdle(device) {
  let art = "none";
  els.renameBtn.hidden = !device || !els.renameInput.hidden;
  if (!els.renameInput.hidden) return;  // don't fight the user while renaming
  els.deviceChipName.textContent = device ? device.name + (device.wifi ? " · Wi-Fi" : "") : "No device connected";
  const NEED = {
    wifi: ["Wi-Fi", "Wi-Fi isn't set up yet",
      "Wireless is chosen. Connect the cable, unlock the device and press Set up Wi-Fi in Settings once, then unplug. The PC and the device must be on the same network. Or switch to Wired."],
    mode: ["Driver", "Choose a connection mode", "Pick Apple Driver or ZenLink Studio to continue."],
    removefilter: ["Unsafe driver", "Remove the libusb0 filter first",
      "A libusb0 filter is stacked on Apple's driver. It can crash Windows and hides the device. Open Connection & Drivers and press Remove libusb0 filter, then reconnect the cable."],
    bind: ["Driver", "Install the ZenLink Studio driver",
      "ZenLink Studio uses the libusb0 driver on the device, like TikTok LIVE Studio. Open Connection & Drivers and press Install ZenLink Studio driver, then reconnect the cable."],
    package: ["Driver", "libusb-win32 isn't installed",
      "ZenLink Studio uses libusb-win32, which isn't on this PC. Install TikTok LIVE Studio's iOS driver or libusb-win32 once, then press Check drivers."],
    restore: ["Driver", "Restore the Apple driver",
      "The device uses the ZenLink Studio driver, so Apple mode can't see it. Open Connection & Drivers and press Restore Apple driver, then reconnect the cable."],
  };
  const need = device && NEED[device.need];
  const idleAction = $("#idle-action");
  idleAction.hidden = !need;
  els.deviceChipStatus.textContent = !device ? "" : need ? need[0] : device.ready ? "Ready" : "Tap Trust";
  if (need) {
    art = "driver";
    els.idleTitle.textContent = need[1];
    els.idleSub.textContent = need[2];
    idleAction.textContent = device.need === "mode" ? "Choose mode" : "Open Connection & Drivers";
  } else if (!device) {
    els.idleTitle.textContent = "Plug in your iPhone or iPad";
    els.idleSub.textContent = state.service === false
      ? "Apple Mobile Device Service isn't running. Install Apple Devices from the Microsoft Store."
      : "Use a USB cable, unlock the device and tap Trust when it asks. Set up Wi-Fi once to cast without the cable.";
  } else if (!device.ready) {
    art = "driver";
    els.idleTitle.textContent = "Tap Trust on the device";
    els.idleSub.textContent = "Unlock it and confirm Trust This Computer with your passcode.";
  } else {
    art = "ready";
    els.idleTitle.textContent = `${device.name} is ready`;
    els.idleSub.textContent = device.wifi ? "Connected over Wi-Fi. Unlock it, then press Start casting."
      : "Unlock it, then press Start casting.";
  }
  els.deviceArt.dataset.state = art;
  els.deviceChip.dataset.state = art;
  renderAirplayGuide(device);
}

// ZenLink Studio over Wi-Fi: the device mirrors to us, so show how.
function renderAirplayGuide(device) {
  const guide = $("#airplay-guide");
  const on = Boolean(device?.airplay) && state.phase !== "streaming";
  guide.hidden = !on;
  if (!on) return;
  $("#ag-name").textContent = `“${device.receiver || "ZenLink"}”`;
  const waiting = CASTING.includes(state.phase);
  els.idleTitle.textContent = waiting ? "Waiting for your iPhone or iPad" : "Mirror over Wi-Fi with AirPlay";
  els.idleSub.textContent = waiting ? "ZenLink is visible on your network now. Follow these steps on the device:"
    : "Press Start casting, then follow these steps on the device:";
  $("#ag-note").textContent = firewallOk === false
    ? "ZenLink doesn't appear in the list? Settings → Connection & Drivers → Allow ZenLink on the network."
    : "No touch control over AirPlay; picture and sound only.";
}

// ---------------------------------------------------------------- stealth dim & passcode

async function toggleStealthDim() {
  if (state.phase !== "streaming" || !appleInput()) return toast("Stealth dim needs Apple Driver mode while casting.");
  const r = await api("/api/keyboard/action", { action: "stealth" }).catch(() => ({ ok: false }));
  if (!r.ok) return toast(r.message || "The device didn't take it.", "bad");
  toast(r.message === "dimmed" ? "Device screen dimmed to 0%. The picture here stays full quality. Alt+D restores it."
    : "Device brightness restored (~25%).");
}

async function refreshPin() {
  try {
    const r = await (await fetch("/api/passcode", { cache: "no-store" })).json();
    $("#pin-input").placeholder = r.set ? "Saved (hidden) — type to replace" : "Not set";
  } catch (_) {}
}
$("#pin-save").addEventListener("click", async () => {
  const value = $("#pin-input").value;
  const r = await api("/api/passcode", { passcode: value });
  $("#pin-input").value = "";
  toast(r.set ? "Passcode saved (encrypted)." : "Passcode removed.");
  refreshPin();
});
refreshPin();

// ---------------------------------------------------------------- copy text from the screen (OCR)

// The current picture, upright, as a PNG (Windows OCR reads up to 2600 px).
function captureScreenPng() {
  const rot = player.appliedRotation || 0, turned = rot === 90 || rot === 270;
  let src, w, h;
  if (player.videoMode) {
    src = $("#screen-video");
    w = src.videoWidth; h = src.videoHeight;
  } else {
    src = els.screen;
    w = src.width; h = src.height;
  }
  if (!w || !h) return null;
  const outW0 = player.videoMode && turned ? h : w, outH0 = player.videoMode && turned ? w : h;
  const scale = Math.min(1, 2600 / Math.max(outW0, outH0));
  const c = document.createElement("canvas");
  c.width = Math.round(outW0 * scale); c.height = Math.round(outH0 * scale);
  const g = c.getContext("2d");
  g.translate(c.width / 2, c.height / 2);
  if (player.videoMode && rot) g.rotate((rot * Math.PI) / 180);
  const dw = (player.videoMode && turned ? c.height : c.width), dh = (player.videoMode && turned ? c.width : c.height);
  g.drawImage(src, -dw / 2, -dh / 2, dw, dh);
  return c.toDataURL("image/png");
}

let ocrBusy = false;
async function copyScreenText() {
  if (ocrBusy) return;
  if (state.phase !== "streaming") return toast("Start casting first.");
  const image = captureScreenPng();
  if (!image) return toast("No picture to read yet.");
  ocrBusy = true;
  toast("Reading the text on the screen…");
  try {
    const r = await api("/api/ocr", { image });
    if (!r.ok) return toast(r.message || "Text recognition failed.", "bad");
    if (!r.text) return toast("No text found on the screen.");
    try { await navigator.clipboard.writeText(r.text); } catch (_) {
      return toast("Text read, but Windows didn't allow copying it to the clipboard.", "bad");
    }
    toast(r.added ? `Copied to the PC: ${r.added} new line${r.added === 1 ? "" : "s"} (${r.total} in total). Alt+R starts over.`
      : "Nothing new on this screen; the copied text is unchanged.");
  } finally {
    ocrBusy = false;
  }
}

// ---------------------------------------------------------------- native sound

function pushAudioSettings() {
  if (!state.nativeAudio) return;
  const muted = els.muteBtn.getAttribute("aria-pressed") === "true";
  api("/api/audio", { enabled: settings.audioOut === "pc", volume: Number(settings.volume) / 100, muted }).catch(() => {});
}

// Sound latency for the stats bar and lip sync (native path).
setInterval(async () => {
  if (!state.nativeAudio || state.phase !== "streaming") return;
  try {
    const a = await (await fetch("/api/audio", { cache: "no-store" })).json();
    if (!a.running) return;
    player._audioLat = a.latencyMs;
    player._audioParts = a;
    player._stats.audio = Math.max(player._stats.audio, 1);
  } catch (_) {}
}, 1000);

// ---------------------------------------------------------------- 1-2-3-4

const FLOW_TEXT = {
  apple: [
    ["Detect device", "Found through Apple Mobile Device Service"],
    ["Secure connection", "Opening an encrypted USB tunnel to the device"],
    ["Prepare the device", "Keep it unlocked. The first time, a small developer image is installed."],
    ["Start stream", "Receiving the screen in HEVC"],
  ],
  airplay: [
    ["Device connected", "Your iPhone/iPad chose ZenLinkStudio in Screen Mirroring"],
    ["Secure connection", "AirPlay pairing and FairPlay key exchange"],
    ["Receive video", "Decrypting the AirPlay stream"],
    ["Start stream", "Receiving the screen over Wi-Fi"],
  ],
  usbdirect: [
    ["Detect device", "Found on USB through the libusb0 driver"],
    ["Open the device", "Switching it to its screen-capture mode"],
    ["Check trust", "Keep it unlocked while ZenLink proves this PC is trusted"],
    ["Start stream", "Receiving the screen over ZenLink Studio"],
  ],
};

function openConnect(name) {
  const texts = (state.devices || [])[0]?.airplay ? FLOW_TEXT.airplay
    : FLOW_TEXT[state.connection?.active] || FLOW_TEXT.apple;
  els.flow.forEach((li, i) => {
    const b = li.querySelector("b"), small = li.querySelector("small");
    if (b && texts[i]) b.textContent = texts[i][0];
    if (small && texts[i]) small.textContent = texts[i][1];
  });
  showingConnect = true;
  flowStep = -1;
  clearTimeout(revealTimer);
  els.connect.removeAttribute("data-error");
  els.connectError.hidden = true;
  els.connectTitle.textContent = `Connecting${name ? ` to ${name}` : ""}`;
  for (const li of els.flow) li.dataset.state = "todo";
  setProgress(0);
  els.idle.hidden = true;
  els.connect.hidden = false;
}

function advanceFlow(step) {
  if (step === undefined || step < flowStep) return;
  flowStep = step;
  els.flow.forEach((li, i) => {
    li.dataset.state = i < step ? "done" : i === step ? "active" : "todo";
  });
  setProgress(Math.min(step, 4) / 4);
  if (step >= 4) {
    els.connectTitle.textContent = "Connected";
    clearTimeout(revealTimer);
    revealTimer = setTimeout(() => {
      els.connect.hidden = true;
      showScreen(true);
    }, 650);
  }
}

function failFlow(message) {
  const i = Math.max(flowStep, 0);
  if (els.flow[i]) els.flow[i].dataset.state = "failed";
  els.connect.dataset.error = "";
  els.connectTitle.textContent = "Couldn't connect";
  els.connectErrorText.textContent = message;
  els.connectError.hidden = false;
}

function closeConnect() {
  if (!showingConnect) return;
  showingConnect = false;
  els.connect.hidden = true;
  if (state.phase !== "streaming") els.idle.hidden = false;
}

function setProgress(fraction) {
  els.progressBar.style.width = `${Math.round(fraction * 100)}%`;
  els.connectPct.textContent = `${Math.round(fraction * 100)}%`;
}

function showScreen(on) {
  els.stage.classList.toggle("live", on);
  els.screen.hidden = !on || player.videoMode;
  $("#screen-video").hidden = !on || !player.videoMode;
  els.idle.hidden = on || showingConnect;
}

// ---------------------------------------------------------------- metrics

function setMetric(el, text) {
  if (el.textContent === text) return;
  el.textContent = text;
  el.classList.remove("flash");
  void el.offsetWidth;
  el.classList.add("flash");
}

// Header: device name and resolution, small, beside "ZenLink · USB / Wi-Fi".
let lastStats = null;
function renderHeaderInfo(stats) {
  lastStats = stats;
  const name = state.device?.name || (state.devices || [])[0]?.name || "";
  const res = stats ? `${stats.width}×${stats.height}` : "";
  $("#hdr-info").textContent = [name, res].filter(Boolean).join(" · ");
  renderHeaderStats(stats);
}

// FPS · video delay · audio delay, colour-coded, beside the device name.
function renderHeaderStats(stats) {
  const el = $("#hdr-stats");
  if (!el) return;
  const on = settings.hdrStats && stats && state.phase === "streaming";
  el.hidden = !on;
  if (!on) return;
  const audio = stats.audioMs ?? (player.nativeAudio && player._audioLat ? Math.round(player._audioLat) : null);
  const cell = (label, value, unit, warn, bad) => {
    const tone = value == null ? "" : value >= bad ? "bad" : value >= warn ? "warn" : "";
    return `<span class="${tone}">${label} <b>${value ?? "—"}</b>${value == null ? "" : unit}</span>`;
  };
  const fpsTone = stats.fps < 25 ? "bad" : stats.fps < 45 ? "warn" : "";
  el.innerHTML = `<span class="${fpsTone}"><b>${stats.fps}</b> fps</span>`
    + cell("video", stats.latency, " ms", 60, 150)
    + cell("audio", audio, " ms", 150, 300);
}

function renderMetrics(stats) {
  renderHeaderInfo(stats);
  const fs = $("#fs-stats");
  if (fs) fs.textContent = stats ? `${stats.width}×${stats.height} · ${stats.fps} fps · ${QUALITY_LABEL[castQuality] || "Best graphics"} ${stats.mbps} Mbps · ${stats.latency ?? "—"} ms` : "";
  const m = els.metrics;
  if (!stats) {
    for (const key of ["res", "fps", "lat", "audio", "rate"]) if (m[key]) m[key].textContent = "—";
    const q = $("#m-quality");
    if (q) q.textContent = QUALITY_LABEL[settings.quality] || "Best graphics";
    return;
  }
  if (m.rate) m.rate.textContent = `${stats.mbps} Mbps`;
  $("#d-res").textContent = `${stats.width}×${stats.height} · ${stats.fps} fps`;
  $("#d-rate").textContent = `${stats.mbps} Mbps`;
  if (player.codecName) $("#d-codec").textContent = player.codecName;
  if (m.rate) m.rate.title = `${QUALITY_LABEL[castQuality] || "Best graphics"} · measured ${stats.mbps} Mbps`;
  const q = $("#m-quality");
  if (q) setMetric(q, QUALITY_LABEL[castQuality] || "Best graphics");
  setMetric(m.res, `${stats.width}×${stats.height}`);
  m.fps.textContent = String(stats.fps);
  m.lat.textContent = stats.latency === null ? "—" : `${stats.latency} ms`;
  setMetric(m.audio, stats.audio ? (stats.audioMs != null ? `${stats.audioMs} ms` : "48 kHz") : "Silent");
  const ap = player._audioParts;
  if (m.audio) m.audio.title = stats.audio
    ? `48 kHz stereo. Audio latency ${stats.audioMs} ms = ZenLink queue ${ap?.queueMs ?? "?"} ms + Windows output ${ap?.outputMs ?? "?"} ms.`
    : "";
}

window.ZenLinkToast = (text) => toast(text);
function toast(text, tone = "") {
  const el = document.createElement("div");
  el.className = "toast";
  el.dataset.tone = tone;
  const body = document.createElement("span");
  body.textContent = text;
  const close = document.createElement("button");
  close.type = "button";
  close.setAttribute("aria-label", "Dismiss");
  close.textContent = "×";
  close.addEventListener("click", () => el.remove());
  el.append(body, close);
  els.toasts.append(el);
  if (tone !== "bad") setTimeout(() => el.remove(), 5000);
}

// ---------------------------------------------------------------- actions

// Quality preset sent with Start (Apple mode; see zenlink/apple/offer.py).
function castOptions() {
  return { quality: settings.quality };
}

// 1:1 shows one stream pixel per screen pixel (scroll/clip if bigger).
function sizeScreen() {
  const c = els.screen;
  if (settings.fit === "native" && c.width) {
    const dpr = window.devicePixelRatio || 1;
    c.style.width = `${c.width / dpr}px`;
    c.style.height = `${c.height / dpr}px`;
  } else {
    c.style.width = c.style.height = "";
  }
}
window.addEventListener("resize", sizeScreen);

const QUALITY_LABEL = { performance: "Performance", normal: "Normal", best: "Best graphics" };
let castQuality = "best"; // the preset the running session was started with

async function startCasting() {
  if (!state.connection?.active) { showChooser(true); return; }
  player.reset();
  castQuality = settings.quality;
  if (!(state.devices || [])[0]?.airplay) {
    openConnect((state.devices || [])[0]?.name);
    advanceFlow(0);
  }
  await api("/api/cast/start", { options: castOptions() });
}

// ---------------------------------------------------------------- input to the device

// Gestures follow PhoneLink V1: mouse = finger, drags may start on the margin
// around the picture (clamped onto the edge), bottom-edge swipe = Home (pause
// before letting go = App Switcher), mouse Back button / Alt+Left = Back
// swipe, trackpad two-finger scroll = finger scroll, sideways = page swipe.

const postCast = (path, payload) =>
  fetch(`/cast/${path}`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(payload) }).catch(() => {});
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const appleInput = () => (state.backend || state.connection?.active) !== "usbdirect";
const screenShown = () => !els.screen.hidden || !$("#screen-video").hidden;
const controlling = () => settings.touch && appleInput() && state.phase === "streaming" && settingsEl.hidden && screenShown();

// Where the picture actually is on screen (the <video> fills the stage and
// letterboxes; the canvas path has its own box).
function pictureRect() {
  if (!player.videoMode) return els.screen.getBoundingClientRect();
  const sr = els.stage.getBoundingClientRect();
  const rot = player.appliedRotation, turned = rot === 90 || rot === 270;
  const w = turned ? player.frameSize.height : player.frameSize.width;
  const h = turned ? player.frameSize.width : player.frameSize.height;
  if (!w || !h) return sr;
  const sc = settings.fit === "cover" ? Math.max(sr.width / w, sr.height / h) : Math.min(sr.width / w, sr.height / h);
  const pw = w * sc, ph = h * sc;
  const left = sr.left + (sr.width - pw) / 2, top = sr.top + (sr.height - ph) / 2;
  return { left, top, right: left + pw, bottom: top + ph, width: pw, height: ph };
}

// One request at a time so a drag never arrives out of order; queued moves
// collapse into the newest.
const touchQueue = [];
let touchBusy = false;
async function sendTouch(type, p, coalesce = false) {
  const last = touchQueue[touchQueue.length - 1];
  const ev = { type, x: p.x, y: p.y, coalesce };
  if (coalesce && last && last.coalesce) touchQueue[touchQueue.length - 1] = ev;
  else touchQueue.push(ev);
  if (touchBusy) return;
  touchBusy = true;
  while (touchQueue.length) {
    const { type: t, x, y } = touchQueue.shift();
    await postCast("touch", { type: t, x, y });
  }
  touchBusy = false;
}

// A scripted finger stroke, queued behind any touches already in flight.
let strokeBusy = false;
async function stroke(from, to, steps, dt, holdMs = 0) {
  if (strokeBusy) return;
  strokeBusy = true;
  try {
    while (touchBusy) await wait(5);
    await postCast("touch", { type: "contact", ...from });
    for (let i = 1; i <= steps; i++) {
      await wait(dt);
      await postCast("touch", {
        type: "contact",
        x: Math.round(from.x + ((to.x - from.x) * i) / steps),
        y: Math.round(from.y + ((to.y - from.y) * i) / steps),
      });
    }
    if (holdMs) {
      await wait(holdMs);
      await postCast("touch", { type: "contact", ...to });
    }
    await postCast("touch", { type: "release", ...to });
  } finally {
    strokeBusy = false;
  }
}

// A point on the picture as the user sees it (u, v in 0..1, (0,0) = top-left
// of the shown picture) in the device's touch space, whatever the rotation.
// Same transform as CastPlayer.touchPoint, so gestures follow the screen.
function devicePoint(u, v) {
  const rot = player.appliedRotation || 0;
  let x = u, y = v;
  if (rot === 90) [x, y] = [v, 1 - u];
  else if (rot === 180) [x, y] = [1 - u, 1 - v];
  else if (rot === 270) [x, y] = [1 - v, u];
  const clamp = (n) => Math.round(Math.min(1, Math.max(0, n)) * 65535);
  return { x: clamp(x), y: clamp(y) };
}
// A double Home press opens the App Switcher (verified on iPadOS 27); an injected
// swipe-up-and-hold is too slow for iOS to read as that gesture.
const openAppSwitcher = () => pressHome(2);
const swipeOnScreen = (u0, v0, u1, v1, steps, dt, holdMs = 0) =>
  stroke(devicePoint(u0, v0), devicePoint(u1, v1), steps, dt, holdMs);
let lastUV = { u: 0.5, v: 0.5 };

async function pressHome(times = 1) {
  for (let i = 0; i < times; i++) {
    if (i) await wait(120);
    await postCast("button", { name: "home", state: "press" });
  }
}
const backGesture = () => swipeOnScreen(0, 0.5, 0.65, 0.5, 14, 16);

// iOS edge gestures (Control Center, Notification Center, Back) only start
// when the contact begins exactly on the edge.
const EDGE = 2500;
const snapToEdge = (c) => ({
  x: c.x < EDGE ? 0 : c.x > 65535 - EDGE ? 65535 : c.x,
  y: c.y < EDGE ? 0 : c.y > 65535 - EDGE ? 65535 : c.y,
});
// Injected touches are too slow for iOS to read a bottom-edge flick as Home,
// so that drag is kept local and turned into the Home button.
const HOME_SWIPE_MIN = 5000;
const SWITCHER_HOLD_MS = 450;
let edgeSwipe = null;

let activePointer = null;
let lastPoint = { x: 32768, y: 32768 };

function releaseActive() {
  if (activePointer === null) return;
  activePointer = null;
  if (edgeSwipe) {
    const swipe = edgeSwipe;
    edgeSwipe = null;
    if (swipe.armed) {
      if (performance.now() - swipe.lastMoveAt > SWITCHER_HOLD_MS) openAppSwitcher();
      else pressHome(1);
    }
    else { sendTouch("contact", swipe.start); sendTouch("release", swipe.start); }
    return;
  }
  sendTouch("release", lastPoint);
}

els.stage.addEventListener("pointerdown", (e) => {
  if (!controlling()) return;
  if (e.button === 3) { e.preventDefault(); backGesture(); return; }
  if (e.button !== 0 || e.target.closest("button, a, input, .idle, .connect")) return;
  // Margins count only close to the picture (drag up from below it for Home).
  const rect = pictureRect();
  const pad = 48;
  if (e.clientX < rect.left - pad || e.clientX > rect.right + pad || e.clientY < rect.top - pad || e.clientY > rect.bottom + pad) return;
  const p = player.touchPoint(e.clientX, e.clientY, true);
  if (!p) return;
  e.preventDefault();
  try { els.stage.setPointerCapture(e.pointerId); } catch (_) {}
  activePointer = e.pointerId;
  lastPoint = snapToEdge(p);
  if (lastPoint.y === 65535 && settings.gestures) {
    edgeSwipe = { start: lastPoint, armed: false, lastMoveAt: performance.now(), lastY: lastPoint.y };
    return;
  }
  sendTouch("contact", lastPoint);
});
els.stage.addEventListener("pointermove", (e) => {
  const r = pictureRect();
  if (r.width && e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) {
    lastUV = { u: (e.clientX - r.left) / r.width, v: (e.clientY - r.top) / r.height };
  }
  if (e.pointerId !== activePointer) return;
  const p = player.touchPoint(e.clientX, e.clientY, true);
  if (!p) return;
  lastPoint = p;
  if (edgeSwipe) {
    if (edgeSwipe.start.y - p.y > HOME_SWIPE_MIN) edgeSwipe.armed = true;
    if (Math.abs(p.y - edgeSwipe.lastY) > 600) { edgeSwipe.lastMoveAt = performance.now(); edgeSwipe.lastY = p.y; }
    return;
  }
  sendTouch("contact", p, true);
});
const endPointer = (e) => {
  if (e.pointerId !== activePointer) return;
  const p = player.touchPoint(e.clientX, e.clientY, true);
  if (p && !edgeSwipe) lastPoint = p;
  releaseActive();
};
els.stage.addEventListener("pointerup", endPointer);
els.stage.addEventListener("pointercancel", endPointer);
els.stage.addEventListener("lostpointercapture", (e) => { if (e.pointerId === activePointer) releaseActive(); });
window.addEventListener("blur", releaseActive);
els.stage.addEventListener("auxclick", (e) => { if (e.button === 3) e.preventDefault(); });
els.screen.addEventListener("contextmenu", (e) => e.preventDefault());
els.stage.addEventListener("contextmenu", (e) => { if (state.phase === "streaming") e.preventDefault(); });

// Trackpad / wheel: vertical becomes a finger scroll near the last touch,
// a sideways two-finger swipe becomes a quick page swipe.
let wheelY = 0, wheelBusy = false, wheelX = 0, wheelXTimer = 0, lastHSwipe = 0;
async function pumpWheel() {
  if (wheelBusy || Math.abs(wheelY) < 18) return;
  wheelBusy = true;
  try {
    const dyIn = wheelY;
    wheelY = 0;
    // Scroll where the pointer is, along the screen's own vertical axis.
    const u = Math.max(0.15, Math.min(0.85, lastUV.u));
    const v = Math.max(0.25, Math.min(0.75, lastUV.v));
    const speed = Number(settings.scrollSpeed) / 100 || 1;
    const dist = Math.max(0.03, Math.min(0.7, Math.max(0.09, Math.min(0.34, Math.abs(dyIn) * 0.0011)) * speed));
    const endV = Math.max(0.04, Math.min(0.96, v + (dyIn > 0 ? -dist : dist)));
    await swipeOnScreen(u, v, u, endV, 5, 10);
  } finally {
    wheelBusy = false;
    if (Math.abs(wheelY) >= 18) pumpWheel();
  }
}
els.stage.addEventListener("wheel", (e) => {
  if (!controlling() || !settings.gestures) return;
  e.preventDefault();
  if (Math.abs(e.deltaX) > Math.abs(e.deltaY) * 1.3) {
    wheelX += e.deltaX;
    clearTimeout(wheelXTimer);
    wheelXTimer = setTimeout(() => { wheelX = 0; }, 200);
    const now = performance.now();
    if (Math.abs(wheelX) >= 28 && now - lastHSwipe > 260) {
      const left = wheelX > 0;
      wheelX = 0;
      lastHSwipe = now;
      swipeOnScreen(left ? 0.84 : 0.16, 0.5, left ? 0.16 : 0.84, 0.5, 7, 10);
    }
    return;
  }
  wheelY += e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY;
  pumpWheel();
}, { passive: false });

window.addEventListener("keydown", (e) => {
  if (e.altKey && e.key === "ArrowLeft" && controlling() && settings.gestures) {
    e.preventDefault();
    e.stopPropagation();
    backGesture();
  }
}, true);

// Keyboard: PhoneLink V1's HID key mapper, sent to the device while live.
window.HandsetKeys?.attach({
  post: (payload) => fetch("/cast/key", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(payload) }),
  enabled: () => settings.keyboard && appleInput() && state.phase === "streaming" && settingsEl.hidden,
});
$("#btn-home")?.addEventListener("click", () => pressHome(1));

// ---------------------------------------------------------------- gestures & buttons panel

// Gestures are defined on the picture as shown (u, v in 0..1) and mapped to
// the device by devicePoint(), so they work in portrait and landscape. iOS
// only starts a system gesture when the contact begins exactly on the edge.
// Home uses the Home button (an injected swipe-up is too slow to read as Home);
// App switcher is the iPadOS gesture: swipe up from the bottom edge and pause.
const PANEL_GESTURES = {
  notifications: [0.22, 0, 0.22, 0.62, 12, 16, 0],
  control: [0.92, 0, 0.92, 0.62, 12, 16, 0],
};
const ctlPanel = $("#ctl-panel");
const ctlBtn = $("#btn-controls");
function openControls(on) {
  ctlPanel.hidden = !on;
  ctlBtn.setAttribute("aria-expanded", String(on));
  $("#ctl-note").hidden = appleInput();
  ctlPanel.classList.toggle("disabled", !appleInput());
}
// Gesture panel visibility (Settings > Controls): "always" docks it open while
// casting in Apple Driver mode, "auto" opens it from the button and hides it on
// an outside click, "hidden" removes the button (G still toggles it).
let ctlDocked = false;
let ctlUserClosed = false;  // the panel button closed the side panel for this cast
function syncControls() {
  const mode = settings.ctlMode || "always";
  ctlBtn.hidden = mode === "hidden";
  if (!CASTING.includes(state.phase)) ctlUserClosed = false;
  ctlPanel.classList.toggle("disabled", !appleInput());
  $("#ctl-note").hidden = appleInput();
  // Docked as a side panel while casting in Apple Driver mode.
  const dock = mode === "always" && appleInput() && CASTING.includes(state.phase) && !ctlUserClosed;
  if (dock === ctlDocked) return;
  ctlPanel.classList.toggle("docked", dock);
  document.body.classList.toggle("ctl-docked", dock);
  openControls(dock);
  ctlDocked = dock;
  requestAnimationFrame(() => window.dispatchEvent(new Event("resize")));
}
ctlBtn.addEventListener("click", (e) => {
  e.stopPropagation();
  if ((settings.ctlMode || "always") === "always" && appleInput() && CASTING.includes(state.phase)) {
    ctlUserClosed = ctlDocked;  // toggle the side panel
    if (!ctlDocked && !ctlPanel.hidden) openControls(false);
    return syncControls();
  }
  openControls(ctlPanel.hidden);
});
document.addEventListener("pointerdown", (e) => {
  if (!ctlPanel.hidden && !ctlDocked && !e.target.closest("#ctl-panel, #btn-controls")) openControls(false);
});
function flash(btn) {
  btn.classList.remove("sent");
  void btn.offsetWidth;
  btn.classList.add("sent");
}
function canSend() {
  if (!appleInput()) { toast("Gestures and buttons need Apple mode."); return false; }
  if (state.phase !== "streaming") { toast("Start casting first."); return false; }
  return true;
}
ctlPanel.querySelectorAll("[data-gesture]").forEach((b) => b.addEventListener("click", () => {
  if (!canSend()) return;
  flash(b);
  const g = b.dataset.gesture;
  if (g === "home") return pressHome(1);
  if (g === "switcher") return openAppSwitcher();
  swipeOnScreen(...PANEL_GESTURES[g]);
}));
ctlPanel.querySelectorAll("[data-swipe]").forEach((b) => b.addEventListener("click", () => {
  if (!canSend()) return;
  flash(b);
  const left = b.dataset.swipe === "left";
  // "Swipe" from the right edge goes forward; "Swipe →" from the left edge is Back.
  swipeOnScreen(left ? 1 : 0, 0.5, left ? 0.12 : 0.88, 0.5, 10, 20);
}));
ctlPanel.querySelectorAll("[data-btn]").forEach((b) => b.addEventListener("click", () => {
  if (!canSend()) return;
  flash(b);
  postCast("button", { name: b.dataset.btn, state: "press" });
}));
// A focused button would swallow Space / Enter meant for the device.
ctlPanel.addEventListener("pointerup", (e) => e.target.closest("button")?.blur());
$("#btn-home")?.addEventListener("contextmenu", (e) => { e.preventDefault(); openAppSwitcher(); });

els.castBtn.addEventListener("click", async () => {
  els.castBtn.disabled = true;
  try {
    if (CASTING.includes(state.phase)) await api("/api/cast/stop");
    else await startCasting();
  } finally {
    applyState(state);
  }
});

els.retryBtn.addEventListener("click", () => startCasting());

// Rename the connected device (stored per device in devices.json).
els.renameBtn.addEventListener("click", () => {
  const device = (state.devices || [])[0];
  if (!device) return;
  els.renameInput.value = device.name;
  els.deviceChipName.hidden = els.deviceChipStatus.hidden = els.renameBtn.hidden = true;
  els.renameInput.hidden = false;
  els.renameInput.focus();
  els.renameInput.select();
});

async function finishRename(save) {
  if (els.renameInput.hidden) return;
  const device = (state.devices || [])[0];
  els.renameInput.hidden = true;
  els.deviceChipName.hidden = els.deviceChipStatus.hidden = false;
  if (save && device) {
    els.deviceChipName.textContent = els.renameInput.value.trim() || device.name;
    applyState(await api("/api/device/name", { id: device.id, name: els.renameInput.value }));
  } else {
    applyState(state);
  }
}
els.renameInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") finishRename(true);
  else if (e.key === "Escape") finishRename(false);
});
els.renameInput.addEventListener("blur", () => finishRename(true));

els.muteBtn.addEventListener("click", () => {
  const muted = els.muteBtn.getAttribute("aria-pressed") !== "true";
  els.muteBtn.setAttribute("aria-pressed", String(muted));
  player.setMuted(muted || settings.audioOut === "device");
  prefs.set("muted", muted);
  pushAudioSettings();
});


const toggleFullscreen = () => window.ZenLinkWindow?.toggleFullscreen();
els.fullscreenBtn.addEventListener("click", toggleFullscreen);
els.stage.addEventListener("dblclick", (e) => {
  // With touch on, a double click is a double tap on the device.
  const r = pictureRect();
  const onPicture = e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
  if (state.phase === "streaming" && !(settings.touch && appleInput() && onPicture)) toggleFullscreen();
});

// ---------------------------------------------------------------- settings

const DEFAULTS = {
  quality: "best", touch: true, keyboard: true, gestures: true, scrollSpeed: "100",
  fit: "contain", fitWindow: true, smoothing: "smooth", rotation: "auto", stats: false, dots: true,
  hdrStats: true, audioOut: "pc", volume: 100, audioBuffer: "low", lipSync: true, closeToTray: false, ctlMode: "always", autoStart: false, onTop: false, theme: "dark",
};
// The stats bar is a debugging aid now: hidden unless turned on again.
if (prefs.get("statsV2", null) === null) { prefs.set("stats", false); prefs.set("statsV2", true); }
// Older builds stored rotation "0" for Auto and a bare bitrate cap.
if (prefs.get("rotationV2", null) === null) { prefs.set("rotation", "auto"); prefs.set("rotationV2", true); }
// Only three presets remain (Auto / Custom were removed: the device ignores higher caps).
if (!["best", "normal", "performance"].includes(prefs.get("quality", "best"))) prefs.set("quality", "best");
const settings = Object.fromEntries(Object.entries(DEFAULTS).map(([k, v]) => [k, prefs.get(k, v)]));
const settingsEl = $("#settings");
const scrim = $("#scrim");

function applySetting(key, value) {
  settings[key] = value;
  prefs.set(key, value);
  const body = document.body;
  switch (key) {
    case "fit": body.dataset.fit = value; sizeScreen(); break;
    case "fitWindow":
      if (value && player.frameSize.width) {
        const turned = player.appliedRotation === 90 || player.appliedRotation === 270;
        const { width, height } = player.frameSize;
        window.ZenLinkWindow?.fitAspect(turned ? height : width, turned ? width : height);
      }
      break;
    case "quality":
      body.dataset.quality = settings.quality;
      if (state.phase !== "streaming") renderMetrics(null);
      if (key === "quality" && state.phase === "streaming") toast("New quality applies the next time you start casting.");
      break;
    case "smoothing": body.dataset.smoothing = value; break;
    case "stats": body.dataset.stats = value ? "on" : "off"; break;
    case "hdrStats": renderHeaderStats(lastStats); break;
    case "dots": body.dataset.dots = value ? "on" : "off"; break;
    case "rotation": player.setRotation(value); break;
    case "audioOut":
      player.setMuted(value === "device" || els.muteBtn.getAttribute("aria-pressed") === "true");
      if (state.nativeAudio) pushAudioSettings();
      else if (state.phase === "streaming") { if (value === "pc") player.startAudio(); else player.stopAudio(); }
      break;
    case "volume":
      els.volume.value = String(value);
      els.volume.style.setProperty("--fill", `${value}%`);
      player.setVolume(Number(value) / 100);
      pushAudioSettings();
      break;
    case "audioBuffer": player.setAudioBuffer(value); break;
    case "lipSync": player.lipSync = Boolean(value); break;
    case "onTop": window.ZenLinkWindow?.setOnTop(value); break;
    case "ctlMode": syncControls(); break;
    case "closeToTray": window.pywebview?.api?.set_close_to_tray?.(Boolean(value)); break;
    case "theme": applyTheme(value); break;
  }
  syncSettingsControls();
}

function syncSettingsControls() {
  settingsEl.querySelectorAll(".seg[data-pref], .quality[data-pref]").forEach((seg) => {
    seg.querySelectorAll("button").forEach((b) =>
      b.setAttribute("aria-pressed", String(String(settings[seg.dataset.pref]) === b.dataset.v))
    );
  });
  settingsEl.querySelectorAll("input.switch[data-pref]").forEach((i) => { i.checked = Boolean(settings[i.dataset.pref]); });
  settingsEl.querySelectorAll("input[type=range][data-pref]").forEach((i) => {
    i.value = settings[i.dataset.pref];
    const min = Number(i.min) || 0, max = Number(i.max) || 100;
    i.style.setProperty("--fill", `${((Number(i.value) - min) / (max - min)) * 100}%`);
  });
}

settingsEl.querySelectorAll(".seg[data-pref], .quality[data-pref]").forEach((seg) =>
  seg.querySelectorAll("button").forEach((b) =>
    b.addEventListener("click", () => applySetting(seg.dataset.pref, b.dataset.v))
  )
);
settingsEl.querySelectorAll("input.switch[data-pref]").forEach((i) =>
  i.addEventListener("change", () => applySetting(i.dataset.pref, i.checked))
);
settingsEl.querySelectorAll("input[type=range][data-pref]").forEach((i) =>
  i.addEventListener("input", () => applySetting(i.dataset.pref, Number(i.value)))
);

function openSettings(on) {
  settingsEl.hidden = !on;
  scrim.hidden = !on;
  if (on) {
    const d = (state.devices || [])[0];
    $("#about").textContent = `ZenLink · wired iOS cast\n${d ? `${d.name} · driver ${d.driver || "—"}` : "No device connected"}`;
    syncSettingsControls();
  }
}
$("#btn-settings").addEventListener("click", () => openSettings(settingsEl.hidden));
$("#settings-close").addEventListener("click", () => openSettings(false));
scrim.addEventListener("click", () => openSettings(false));
$("#fs-mute").addEventListener("click", () => els.muteBtn.click());

// The toolbar volume slider and the settings slider are the same setting.
els.volume.addEventListener("input", () => applySetting("volume", Number(els.volume.value)));

// Auto start: when a ready device appears while idle.
let autoStartedFor = null;
function maybeAutoStart() {
  const d = (state.devices || [])[0];
  if (!state.connection?.active) return;  // nothing connects before a mode is chosen
  if (!settings.autoStart || !d || !d.ready) { if (!d) autoStartedFor = null; return; }
  if (state.phase === "idle" && autoStartedFor !== d.id) {
    autoStartedFor = d.id;
    startCasting();
  }
}

// ---------------------------------------------------------------- connection mode & drivers

const MODE_NAME = { apple: "Apple Driver (Standard)", usbdirect: "ZenLink Studio" };
const chooser = $("#chooser");
const chooserContinue = $("#chooser-continue");
const chooserCancel = $("#chooser-cancel");
let chooserPick = null;
let chooserShownOnce = false;

function showChooser(on, { cancellable = false } = {}) {
  chooser.hidden = !on;
  chooserCancel.hidden = !cancellable;
  if (on) {
    const c = state.connection || {};
    chooserPick = c.pending || c.active || null;
    $("#chooser-remember").checked = Boolean(c.remember);
    chooserLink = c.link || "wired";
    syncChooser();
  }
}
let chooserLink = "wired";
function syncLinkSegs(link) {
  document.querySelectorAll(".link-seg button").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.link === link)));
}
function syncChooser() {
  $("#chooser-link").hidden = !chooserPick;
  syncLinkSegs(chooserLink);
  chooser.querySelectorAll(".mode-card").forEach((card) => {
    const on = card.dataset.mode === chooserPick;
    card.setAttribute("aria-checked", String(on));
    card.classList.toggle("selected", on);
  });
  chooserContinue.disabled = !chooserPick;
}
chooser.querySelectorAll(".mode-card").forEach((card) => card.addEventListener("click", () => {
  chooserPick = card.dataset.mode;  // selecting never touches drivers
  syncChooser();
}));
chooserCancel.addEventListener("click", () => showChooser(false));
$("#chooser-link").querySelectorAll("button").forEach((b) => b.addEventListener("click", () => {
  chooserLink = b.dataset.link;
  syncLinkSegs(chooserLink);
}));
document.querySelectorAll(".link-row .link-seg button").forEach((b) => b.addEventListener("click", async () => {
  if (CASTING.includes(state.phase)) return toast("Stop casting before switching between wired and wireless.");
  const snap = await api("/api/connection", { link: b.dataset.link });
  if (snap.phase !== undefined) applyState(snap);
  toast(b.dataset.link === "wireless" ? "Wireless: ZenLink casts over Wi-Fi." : "Wired: ZenLink casts over the cable.");
}));
chooserContinue.addEventListener("click", async () => {
  if (!chooserPick) return;
  chooserContinue.disabled = true;
  const wasActive = state.connection?.active;
  const casting = CASTING.includes(state.phase);
  if (wasActive && wasActive !== chooserPick && casting &&
      !confirm("Casting will stop. The new connection mode applies after ZenLink restarts. Continue?")) {
    chooserContinue.disabled = false;
    return;
  }
  if (chooserLink !== state.connection?.link) await api("/api/connection", { link: chooserLink });
  const snap = await api("/api/connection", { mode: chooserPick, remember: $("#chooser-remember").checked });
  showChooser(false);
  if (snap.connection?.restartRequired || snap.restartRequired) {
    offerRestart(`Switch to ${MODE_NAME[chooserPick]}`,
      "ZenLink applies the new connection mode after a restart. If the device's driver must change too, Settings → Connection & Drivers does it first.");
  } else {
    const dev = (snap.drivers?.devices || [])[0];
    const airplay = chooserPick === "usbdirect" && chooserLink === "wireless";
    const mismatch = dev && !airplay && (dev.unsafeFilter || (chooserPick === "usbdirect" && dev.mode !== "replaced") ||
                             (chooserPick === "apple" && dev.mode === "replaced"));
    if (mismatch) {
      openSettings(true);
      $("#conn-section")?.scrollIntoView({ block: "start" });
      toast("This device's driver doesn't match the chosen mode yet. See Connection & Drivers.");
    }
  }
  if (snap.phase !== undefined) applyState(snap);
});
$("#conn-change").addEventListener("click", () => { openSettings(false); showChooser(true, { cancellable: true }); });
$("#idle-action").addEventListener("click", () => {
  const device = (state.devices || [])[0];
  if (device?.need === "mode") return showChooser(true);
  openSettings(true);
  $("#conn-section")?.scrollIntoView({ block: "start" });
});

function yes(ok, good, bad) { return ok ? good : bad; }
function renderConnection() {
  const c = state.connection || {};
  if (c.needsChoice && !chooserShownOnce) { chooserShownOnce = true; showChooser(true); }
  document.body.dataset.mode = c.active || "none";
  const usb = c.active === "usbdirect";
  $("#usb-quality-note").hidden = !usb;
  $("#quality-row").hidden = usb;
  $("#usb-control-note").hidden = !usb;
  $("#btn-home").hidden = usb;
  $("#source-tag").textContent = usb ? (c.link === "wireless" ? "Studio · AirPlay" : "Studio · USB") : state.device?.wifi || (state.devices || [])[0]?.wifi ? "Wi-Fi" : c.active === "apple" ? "USB" : "iOS";
  syncControls();
  $("#conn-mode-name").textContent = MODE_NAME[c.active] || "No mode chosen";
  if (chooser.hidden) syncLinkSegs(c.link || "wired");
  $(".link-row").hidden = !c.active;
  $(".wifi-row").hidden = c.active !== "apple";
  syncFirewall(c.active === "usbdirect" && c.link === "wireless");
  $("#link-help").textContent = c.active === "usbdirect"
    ? "Wired uses the cable and the libusb0 driver. Wireless is AirPlay: on the device open Control Center → Screen Mirroring → ZenLink."
    : "Wired uses the cable. Wireless casts over Wi-Fi after the one-time setup below.";
  $("#conn-mode-icon").dataset.mode = c.active || "";
  $("#conn-mode-note").textContent = c.restartRequired
    ? `${MODE_NAME[c.pending]} is saved and applies after a restart.`
    : c.remember ? "Remembered for next start." : "Asked again at next start.";
  const dr = state.drivers || {};
  const lib = dr.libusb0 || {};
  const dev = (dr.devices || [])[0];
  $("#d-amds").textContent = dr.apple ? yes(dr.apple.running, "Running", dr.apple.service ? "Stopped" : "Not installed") : "—";
  $("#d-apple").textContent = !dev ? "No device" : dev.mode === "replaced" ? "Replaced by libusb0" : dev.appleReady ? `In use (${dev.service})` : dev.service || "Unknown";
  $("#d-libusb").textContent = lib.installed ? "Installed (libusb0.sys + libusb0.dll)" : lib.bundled ? "Bundled with ZenLink (installs with the driver)" : lib.service ? "Incomplete" : "Not available";
  $("#d-device").textContent = dev ? `${dev.family} · ${dev.status || "connected"}` : "Not connected";
  const directText = {
    replaced: "Installed (libusb0 is the device driver)",
    apple: dev?.libusbPackage ? `Ready to install (${dev.libusbPackage})` : lib.available ? "Ready to install (package is built on install)" : "libusb-win32 not on this PC",
    coexist: "UNSAFE: libusb0 filter on Apple driver",
    unknown: "Unknown driver state",
  };
  $("#d-direct").textContent = dev ? directText[dev.mode] : "—";
  $("#d-apple").textContent = !dev ? "No device" : dev.mode === "replaced" ? "Not in use (ZenLink Studio driver)" : dev.unsafeFilter ? "In use, with an unsafe libusb0 filter" : dev.appleReady ? `In use (${dev.service})` : dev.service || "Unknown";
  const cap = state.capture;
  $("#d-backend").textContent = cap ? MODE_NAME[cap.backend] : state.phase === "idle" ? "Idle" : state.phase;
  $("#d-codec").textContent = cap?.codec || player.codecName || "—";
  // driver buttons follow the device's state and the chosen mode
  const warn = $("#conn-warn");
  let note = "";
  $("#drv-removefilter").hidden = !(dev && dev.unsafeFilter);
  $("#drv-bind").hidden = !(dev && !dev.unsafeFilter && dev.mode === "apple" && lib.available);
  $("#drv-bind").textContent = "Switch to ZenLink Studio driver (libusb0)";
  $("#drv-restore").textContent = "Switch back to Apple driver";
  $("#drv-zadig").hidden = !(dev && lib.zadig && c.active === "usbdirect" && dev.mode !== "replaced");
  $("#drv-restore").hidden = !(dev && !dev.unsafeFilter && (dev.mode === "replaced" || dev.mode === "unknown"));
  if (dev && dev.unsafeFilter) {
    note = "A libusb0 filter is stacked on Apple's driver. This combination can crash Windows and stops Apple mode from seeing the device. Remove it before anything else.";
  } else if (dev && c.active === "usbdirect" && dev.mode === "apple" && !lib.available) {
    note = "ZenLink Studio uses libusb-win32, which isn't on this PC. Install TikTok LIVE Studio's iOS driver or libusb-win32 once, then press Check drivers.";
  } else if (dev && c.active === "usbdirect" && dev.mode === "apple") {
    note = "ZenLink Studio is chosen: install its driver to use it.";
  } else if (dev && c.active === "apple" && dev.mode === "replaced") {
    note = "Apple mode is chosen but the device uses the ZenLink Studio driver: restore the Apple driver to use it.";
  }
  warn.textContent = note;
  warn.hidden = !note;
}

const DRIVER_CONFIRM = {
  bind: "Install the ZenLink Studio driver on this device?\n\nLike TikTok LIVE Studio, ZenLink installs the libusb-win32 driver as the device's driver. While it is installed, Apple mode and Apple apps can't see the device; press Restore Apple driver to switch back.\n\nIf there is no driver package for this model yet, ZenLink builds one the way TikTok LIVE Studio and Zadig do: it signs the package with a new self-signed certificate, deletes that certificate's private key, and adds its public part (\"ZenLink ZenLink Studio\") to Windows' trusted certificates so the driver can install.\n\nA backup is saved first, and Apple's driver is put back automatically if the device doesn't start.\n\nWindows will ask for administrator permission. Unplug and reconnect the cable afterwards.",
  restore: "Restore Apple's driver on this device?\n\nZenLink Studio stops working until its driver is installed again. A backup is saved first. Windows will ask for administrator permission. Unplug and reconnect the cable afterwards.",
  removefilter: "Remove the libusb0 filter from this device?\n\nIt is stacked on Apple's driver, which can crash Windows. Apple's driver stays. A backup is saved first. Windows will ask for administrator permission.",
};
function showBusy(title, text, actions = null) {
  const busy = $("#busy");
  if (!title) { busy.hidden = true; return; }
  $("#busy-title").textContent = title;
  $("#busy-text").textContent = text || "";
  $("#busy-spin").hidden = Boolean(actions);
  const box = $("#busy-actions");
  box.replaceChildren();
  for (const [label, cls, fn] of actions || []) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = `btn ${cls}`;
    b.textContent = label;
    b.addEventListener("click", fn);
    box.append(b);
  }
  busy.hidden = false;
}

async function restartApp() {
  showBusy("Restarting ZenLink…", "Unplug the cable and plug it in again while ZenLink restarts.");
  try { await api("/api/app/restart", {}); } catch (_) {}
}

function offerRestart(title, text) {
  showBusy(title, text, [
    ["Restart ZenLink now", "primary", restartApp],
    ["Later", "ghost-btn", () => showBusy(null)],
  ]);
}

async function driverAction(action) {
  const dev = (state.drivers?.devices || [])[0];
  if (!dev) return toast("Connect the device first.");
  if (CASTING.includes(state.phase)) return toast("Stop casting before changing drivers.");
  if (!confirm(DRIVER_CONFIRM[action])) return;
  showBusy("Changing the device's driver…",
    "Approve the Windows permission prompt. A PowerShell window shows each step; keep the cable plugged in until it finishes.");
  let r;
  try {
    r = await api("/api/drivers/action", { action, instance: dev.instance, confirmed: true });
  } catch (error) {
    r = { ok: false, message: String(error) };
  }
  await refreshDrivers();
  if (r.ok) {
    const toUsb = action === "bind";
    // Keep the connection mode in step with the driver that is now installed.
    if (action !== "removefilter") {
      await api("/api/connection", { mode: toUsb ? "usbdirect" : "apple", remember: state.connection?.remember ?? true });
    }
    offerRestart("Driver changed",
      `${r.message} ZenLink needs a restart to use ${toUsb ? "ZenLink Studio" : "Apple Driver"} mode. Unplug the cable and plug it in again while it restarts.`);
  } else {
    showBusy("Driver not changed", `${r.message}${r.rolledBack ? " The previous state was restored." : ""}`,
      [["OK", "ghost-btn", () => showBusy(null)]]);
  }
}
async function refreshDrivers() {
  try {
    const dr = await (await fetch("/api/drivers", { cache: "no-store" })).json();
    state.drivers = dr;
    renderConnection();
  } catch (_) {}
}
$("#drv-check").addEventListener("click", async () => { await refreshDrivers(); toast("Driver status refreshed."); });
$("#drv-bind").addEventListener("click", () => driverAction("bind"));
$("#drv-restore").addEventListener("click", () => driverAction("restore"));
$("#drv-removefilter").addEventListener("click", () => driverAction("removefilter"));
$("#drv-zadig").addEventListener("click", async () => {
  if (!confirm("Open the official Zadig download page to install the driver by hand?\n\nIn Zadig: Options > List All Devices, pick your iPhone/iPad (Composite Parent), choose libusb-win32 and press Replace Driver. Windows will ask for administrator permission. Afterwards unplug and reconnect the cable and press Check drivers.")) return;
  const r = await api("/api/drivers/zadig", { confirmed: true });
  if (!r.ok) toast(r.message, "bad");
});

// ---------------------------------------------------------------- shortcuts

document.addEventListener("keydown", (e) => {
  if (e.target.closest("input:not([type=range]):not([type=checkbox])")) return;
  const typingToDevice = settings.keyboard && state.phase === "streaming" && settingsEl.hidden;
  const appShortcut = ["F11", "Escape"].includes(e.key) || (e.key === "," && e.ctrlKey) ||
    (e.altKey && !e.ctrlKey && ["KeyH", "KeyC", "KeyR", "KeyD"].includes(e.code));
  if (typingToDevice && !appShortcut) return;
  if (e.key === "F11") {
    e.preventDefault();
    toggleFullscreen();
  } else if (e.key === "Escape") {
    if (!chooser.hidden && !chooserCancel.hidden) showChooser(false);
    else if (!ctlPanel.hidden) openControls(false);
    else if (!settingsEl.hidden) openSettings(false);
    else window.ZenLinkWindow?.exitFullscreen();
  } else if (e.altKey && !e.ctrlKey && e.code === "KeyC") {
    e.preventDefault();
    copyScreenText();
  } else if (e.altKey && !e.ctrlKey && e.code === "KeyR") {
    e.preventDefault();
    api("/api/ocr/clear", {}).then(() => toast("Copy history cleared. The next Alt+C starts fresh."));
  } else if (e.altKey && !e.ctrlKey && e.code === "KeyD") {
    e.preventDefault();
    toggleStealthDim();
  } else if (e.altKey && (e.key === "h" || e.key === "H")) {
    e.preventDefault();
    const order = ["auto", "90", "270"];
    const next = order[(order.indexOf(String(settings.rotation)) + 1) % order.length];
    applySetting("rotation", next);
    toast(next === "auto" ? "Rotation: follows the device" : `Rotation: ${next}°`);
  } else if (e.key === "," && e.ctrlKey) {
    e.preventDefault();
    openSettings(settingsEl.hidden);
  } else if ((e.key === "g" || e.key === "G") && !e.ctrlKey && !e.altKey) {
    ctlBtn.click();
  } else if ((e.key === "m" || e.key === "M") && !e.ctrlKey) {
    els.muteBtn.click();
  } else if (e.key === " " && settingsEl.hidden) {
    e.preventDefault();
    els.castBtn.click();
  }
});

// ---------------------------------------------------------------- theme

function applyTheme(theme) {
  // Switch without transitions, or controls keep the old theme's fill.
  document.documentElement.classList.add("theme-switching");
  document.documentElement.dataset.theme = theme;
  requestAnimationFrame(() =>
    requestAnimationFrame(() => document.documentElement.classList.remove("theme-switching"))
  );
  prefs.set("theme", theme);
}
els.themeBtn.addEventListener("click", () =>
  applySetting("theme", document.documentElement.dataset.theme === "dark" ? "light" : "dark")
);
for (const [k, v] of Object.entries(settings)) applySetting(k, v);

connect();

document.getElementById("support-btn")?.addEventListener("click", () => {
  fetch("/api/support", { method: "POST" }).catch(() => {});
});

document.getElementById("wifi-setup")?.addEventListener("click", async (e) => {
  const btn = e.currentTarget;
  btn.disabled = true;
  toast("Setting up Wi-Fi… keep the device unlocked and on the cable.");
  try {
    const r = await api("/api/wifi/prepare", {});
    if (!r.ok) return toast(r.message || "Wi-Fi setup failed.", "bad");
    toast(`Wi-Fi is ready for ${r.device}. Unplug the cable; it shows up as “Wi-Fi” when both are on the same network.`);
  } catch (_) {
    toast("Wi-Fi setup failed.", "bad");
  } finally {
    btn.disabled = false;
  }
});

// Keep the docked side panel between the top bar and the bottom bar.
{
  const top = document.querySelector(".topbar"), foot = document.querySelector("footer.controls");
  const sync = () => {
    const root = document.documentElement.style;
    root.setProperty("--top-h", `${top && top.offsetParent !== null ? top.getBoundingClientRect().bottom : 0}px`);
    root.setProperty("--foot-h", `${foot && foot.offsetParent !== null ? window.innerHeight - foot.getBoundingClientRect().top : 0}px`);
  };
  const ro = new ResizeObserver(sync);
  [top, foot, document.body].forEach((el) => el && ro.observe(el));
  window.addEventListener("resize", sync);
  sync();
}

// ZenLink Studio over Wi-Fi (AirPlay) needs a Windows Firewall rule so the
// iPhone/iPad can reach ZenLink; Windows often never asks on a Public network.
var firewallOk = null, firewallChecking = false;  // var: read by renderAirplayGuide before this line runs
async function syncFirewall(airplay) {
  const btn = $("#fw-allow");
  if (!airplay) { btn.hidden = true; return; }
  if (firewallOk === null && !firewallChecking) {
    firewallChecking = true;
    try { firewallOk = (await (await fetch("/api/firewall", { cache: "no-store" })).json()).ok; } catch (_) { firewallOk = false; }
    firewallChecking = false;
  }
  btn.hidden = firewallOk !== false;
}
$("#fw-allow").addEventListener("click", async (e) => {
  e.preventDefault();
  toast("Approve the Windows permission prompt to let your iPhone/iPad reach ZenLink over Wi-Fi.");
  const r = await api("/api/firewall/allow", {});
  firewallOk = Boolean(r.ok);
  syncFirewall(true);
  toast(r.ok ? "ZenLink is allowed on the local network. Start casting, then pick ZenLink in Screen Mirroring." : r.message, r.ok ? undefined : "bad");
});


