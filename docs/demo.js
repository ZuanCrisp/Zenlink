// ZenLink live demo: gestures on a mirrored iPhone, iPad, Android phone or
// Android tablet. A simulation of the app window, not a recording.
(() => {
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
  const app = $("#demo-app"), device = $("#demo-device"), screen = $("#demo-screen");
  if (!app) return;
  const lang = () => (document.documentElement.lang === "en" ? "en" : "id");
  const T = {
    home: { id: "Home: usap ke atas dari bawah", en: "Home: swipe up from the bottom" },
    switcher: { id: "App Switcher: Home dua kali", en: "App Switcher: Home twice" },
    notif: { id: "Notifikasi · <kbd>Alt+N</kbd>", en: "Notifications · <kbd>Alt+N</kbd>" },
    cc: { id: "Pusat Kontrol · <kbd>Alt+Q</kbd>", en: "Control Center · <kbd>Alt+Q</kbd>" },
    next: { id: "Usap ← halaman berikutnya", en: "Swipe ← next page" },
    prev: { id: "Usap → halaman sebelumnya", en: "Swipe → previous page" },
    volup: { id: "Volume +", en: "Volume +" },
    voldown: { id: "Volume −", en: "Volume −" },
    lock: { id: "Kunci layar · <kbd>Alt+L</kbd>", en: "Lock screen · <kbd>Alt+L</kbd>" },
    tap: { id: "Klik mouse = sentuhan di layar", en: "Mouse click = a tap on the screen" },
    studio: { id: "Gambar & suara saja: kualitas untuk live", en: "Picture & sound only: quality for live" },
    closed: { id: "Tekan lagi = tutup", en: "Press again = close" },
    aback: { id: "Kembali · <kbd>Alt+←</kbd>", en: "Back · <kbd>Alt+←</kbd>" },
    ahome: { id: "Home", en: "Home" },
    arecents: { id: "Aplikasi terbaru · <kbd>Alt+S</kbd>", en: "Recent apps · <kbd>Alt+S</kbd>" },
    anotif: { id: "Notifikasi · <kbd>Alt+N</kbd>", en: "Notifications · <kbd>Alt+N</kbd>" },
    aqs: { id: "Setelan cepat · <kbd>Alt+Q</kbd>", en: "Quick settings · <kbd>Alt+Q</kbd>" },
    avolup: { id: "Volume + · <kbd>Alt+↑</kbd>", en: "Volume + · <kbd>Alt+↑</kbd>" },
    avoldown: { id: "Volume − · <kbd>Alt+↓</kbd>", en: "Volume − · <kbd>Alt+↓</kbd>" },
    apower: { id: "Tombol power · <kbd>Alt+L</kbd>", en: "Power button · <kbd>Alt+L</kbd>" },
  };

  // --- app icons -------------------------------------------------------
  const P = {
    msg: "M4 5h16v10H9l-5 4z", cam: "M4 8h3l2-3h6l2 3h3v11H4zM12 16a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
    music: "M9 18V6l10-2v12M9 18a2 2 0 1 1-4 0 2 2 0 0 1 4 0zM19 16a2 2 0 1 1-4 0 2 2 0 0 1 4 0z",
    play: "M8 5v14l11-7z", map: "M12 21s-7-6-7-11a7 7 0 0 1 14 0c0 5-7 11-7 11zM12 12a2 2 0 1 0 0-4 2 2 0 0 0 0 4z",
    mail: "M3 6h18v12H3zM3 7l9 6 9-6", gear: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M5.6 18.4 7 17M17 7l1.4-1.4",
    clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3 2", sun: "M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM12 2v2M12 20v2M2 12h2M20 12h2",
    note: "M6 3h9l3 3v15H6zM9 9h6M9 13h6M9 17h4", heart: "M12 20s-7-4.5-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.5-7 10-7 10z",
    phone: "M5 4h4l2 5-2.5 1.5a11 11 0 0 0 5 5L15 13l5 2v4a2 2 0 0 1-2 2A16 16 0 0 1 3 6a2 2 0 0 1 2-2z",
    compass: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM15.5 8.5l-2 5-5 2 2-5z", cal: "M4 6h16v14H4zM4 10h16M8 3v4M16 3v4",
    photo: "M4 5h16v14H4zM4 16l5-5 4 4 3-3 4 4", live: "M2 12a10 10 0 0 1 20 0M6 12a6 6 0 0 1 12 0M12 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2z",
    game: "M6 9h12a3 3 0 0 1 3 3v2a3 3 0 0 1-5 2l-1-1H9l-1 1a3 3 0 0 1-5-2v-2a3 3 0 0 1 3-3zM8 11v3M6.5 12.5h3",
    files: "M3 6h7l2 2h9v11H3z", store: "M5 8h14l-1 12H6zM9 8a3 3 0 0 1 6 0",
  };
  const G = ["#ff4fa3,#c2185b", "#3ddc84,#1f9d57", "#5aa8ff,#2f6fd6", "#ffb020,#e07b00", "#b48cff,#6b3fd6", "#ff6b6b,#d63a3a", "#2fd6c8,#178f86", "#8e8e9a,#55555e"];
  const APPS = [
    ["Pesan", "msg", 1], ["Kamera", "cam", 7], ["Musik", "music", 5], ["Video", "play", 0], ["Peta", "map", 2], ["Mail", "mail", 2],
    ["Kalender", "cal", 5], ["Foto", "photo", 3], ["Jam", "clock", 7], ["Cuaca", "sun", 2], ["Catatan", "note", 3], ["Kesehatan", "heart", 0],
    ["LIVE", "live", 0], ["Game", "game", 4], ["Safari", "compass", 2], ["Setelan", "gear", 7],
    ["Kontak", "phone", 1], ["Buku", "note", 3], ["Podcast", "live", 4], ["Kompas", "compass", 7],
  ];
  const icon = (name, glyph, g, label = true) =>
    `<div class="icon"><i style="background:linear-gradient(145deg,${G[g]})"><svg viewBox="0 0 24 24"><path d="${P[glyph]}"/></svg></i>${label ? `<span>${name}</span>` : ""}</div>`;
  $("#demo-page1").innerHTML = APPS.slice(0, 20).map((a) => icon(...a)).join("");
  $("#demo-page2").innerHTML = APPS.slice(10, 20).concat(APPS.slice(0, 6)).map((a) => icon(...a)).join("");
  $("#demo-dock").innerHTML = [["Telepon", "phone", 1], ["Safari", "compass", 2], ["Pesan", "msg", 1], ["Musik", "music", 5]].map((a) => icon(...a, false)).join("");

  // Android: themed (tonal) round icons.
  const TONE = [["#c8f5dc", "#0b3d24"], ["#cfe7ff", "#0b2d55"], ["#ffd9e8", "#5a0d30"], ["#e5dcff", "#2a1466"], ["#ffe6c2", "#4d2c00"], ["#d2f1f5", "#08393f"]];
  const AAPPS = [
    ["LIVE", "live", 2], ["Kamera", "cam", 1], ["Foto", "photo", 3], ["Musik", "music", 4], ["Peta", "map", 0], ["Kalender", "cal", 5], ["Game", "game", 3],
    ["File", "files", 4], ["Jam", "clock", 5], ["Cuaca", "sun", 1], ["Catatan", "note", 0], ["Store", "store", 2], ["Video", "play", 2], ["Setelan", "gear", 5],
  ];
  const aicon = (name, glyph, t, label = true) =>
    `<div class="a-icon"><i style="background:${TONE[t][0]}"><svg viewBox="0 0 24 24" style="stroke:${TONE[t][1]}"><path d="${P[glyph]}"/></svg></i>${label ? `<span>${name}</span>` : ""}</div>`;
  const fillGrid = () => {
    const n = device.classList.contains("atab") ? 14 : 8;
    $("#da-grid").innerHTML = AAPPS.slice(0, n).map((a) => aicon(...a)).join("");
  };
  $("#da-dock").innerHTML = [["Telepon", "phone", 0], ["Pesan", "msg", 1], ["Browser", "compass", 1], ["Kamera", "cam", 3]].map((a) => aicon(...a, false)).join("");

  // --- clock -----------------------------------------------------------
  const tick = () => {
    const d = new Date(), loc = lang() === "en" ? "en-US" : "id-ID";
    const t = d.toLocaleTimeString(loc, { hour: "2-digit", minute: "2-digit", hour12: false }).replace(".", ":");
    $$("[data-clock]").forEach((el) => { el.textContent = t; });
    const day = d.toLocaleDateString(loc, { weekday: "long", day: "numeric", month: "long" });
    $$("[data-date]").forEach((el) => { el.textContent = day; });
  };
  tick(); setInterval(tick, 15000);

  // --- live stats in the header (simulated) --------------------------------
  let dev = "iphone", link = "usb";
  const isAndroid = () => dev === "android" || dev === "atab";
  setInterval(() => {
    if (document.hidden) return;
    const r = (a, b) => Math.round(a + Math.random() * (b - a));
    const wifi = link === "airplay" || link === "wifi";
    const fps = r(57, 60);
    const video = isAndroid() ? (wifi ? r(24, 38) : r(9, 15)) : (wifi ? r(13, 19) : r(1, 3));
    const audio = isAndroid() ? (wifi ? r(60, 74) : r(40, 48)) : (wifi ? r(140, 165) : r(22, 25));
    $("#st-fps").textContent = fps; $("#st-video").textContent = video; $("#st-audio").textContent = audio;
    $("#st-audio").parentElement.classList.toggle("warn", audio >= 150);
  }, 900);

  // --- shared bits -----------------------------------------------------------
  const caption = $("#demo-caption");
  const say = (key) => { caption.innerHTML = T[key][lang()]; };
  const cursor = $("#demo-cursor"), trail = $("#demo-trail");
  let timers = [];
  const later = (fn, ms) => timers.push(setTimeout(fn, ms));
  const show = (id, on = true) => $("#" + id).classList.toggle("show", on);
  const clearAll = () => {
    timers.forEach(clearTimeout); timers = [];
    ["demo-cc", "demo-nc", "demo-switcher", "demo-vol", "demo-lock", "demo-dim", "da-dim", "da-shade", "da-recents", "da-vol", "da-off"].forEach((k) => show(k, false));
    $("#da-shade").classList.remove("full");
    $("#demo-appwin").classList.remove("open"); $("#da-app").classList.remove("open");
    $("#demo-home").style.transform = ""; $("#da-home").style.transform = "";
    cursor.classList.remove("show", "tapping"); trail.classList.remove("go"); $("#da-back").classList.remove("go");
  };
  const moveCursor = (xPct, yPct) => { cursor.style.left = xPct + "%"; cursor.style.top = yPct + "%"; };
  const swipeUp = () => { trail.classList.add("go"); later(() => trail.classList.remove("go"), 520); };
  const tapOn = (target, win, home) => {
    const sr = screen.getBoundingClientRect(), r = target.getBoundingClientRect();
    const x = ((r.left + r.width / 2 - sr.left) / sr.width) * 100, y = ((r.top + r.height / 2 - sr.top) / sr.height) * 100;
    moveCursor(60, 88); cursor.classList.add("show");
    later(() => moveCursor(x, y), 80);
    later(() => cursor.classList.add("tapping"), 820);
    later(() => {
      win.style.setProperty("--ox", x + "%"); win.style.setProperty("--oy", y + "%"); win.classList.add("open");
      home.style.transform = "scale(.92)"; cursor.classList.remove("tapping");
    }, 1000);
    later(() => cursor.classList.remove("show"), 1900);
  };

  // --- Apple gestures ----------------------------------------------------------
  let page = 0;
  const setPage = (p) => {
    page = p; $("#demo-pages").style.transform = `translateX(${-50 * p}%)`;
    $$("#demo-dots i").forEach((d, i) => d.classList.toggle("on", i === p));
  };
  const apple = {
    tap() {
      clearAll(); say("tap"); setPage(0);
      const icons = $$("#demo-page1 .icon i");
      tapOn(icons[12] || icons[0], $("#demo-appwin"), $("#demo-home"));
    },
    home() {
      const w = $("#demo-appwin");
      if (!w.classList.contains("open")) { apple.tap(); later(() => apple.home(), 2300); return; }
      say("home"); swipeUp();
      later(() => { w.classList.remove("open"); $("#demo-home").style.transform = ""; }, 260);
    },
    switcher() { clearAll(); say("switcher"); swipeUp(); later(() => show("demo-switcher"), 220); },
    notif() { clearAll(); say("notif"); show("demo-nc"); },
    cc() { clearAll(); say("cc"); show("demo-dim"); show("demo-cc"); },
    next() { clearAll(); say("next"); setPage(1); },
    prev() { clearAll(); say("prev"); setPage(0); },
    volup() { volume("demo-vol", true, "volup"); },
    voldown() { volume("demo-vol", false, "voldown"); },
    lock() { clearAll(); say("lock"); show("demo-lock"); },
  };
  const volume = (id, up, key) => {
    clearAll(); say(key);
    const v = $("#" + id); const cur = parseInt(getComputedStyle(v).getPropertyValue("--v")) || 55;
    v.style.setProperty("--v", Math.max(8, Math.min(100, cur + (up ? 18 : -18))) + "%"); show(id);
    later(() => show(id, false), 1400);
  };

  // --- Android gestures --------------------------------------------------------
  const droid = {
    tap() {
      clearAll(); say("tap");
      tapOn($$("#da-grid .a-icon i")[0], $("#da-app"), $("#da-home"));
    },
    back() {
      const w = $("#da-app");
      const open = w.classList.contains("open") || ["da-shade", "da-recents"].some((k) => $("#" + k).classList.contains("show"));
      if (!open) { droid.tap(); later(() => droid.back(), 2300); return; }
      say("aback"); $("#da-back").classList.add("go");
      later(() => { clearAll(); say("aback"); }, 380);
    },
    home() {
      const w = $("#da-app");
      if (!w.classList.contains("open")) { droid.tap(); later(() => droid.home(), 2300); return; }
      say("ahome"); swipeUp();
      later(() => { w.classList.remove("open"); $("#da-home").style.transform = ""; }, 260);
    },
    recents() { clearAll(); say("arecents"); swipeUp(); later(() => show("da-recents"), 220); },
    notif() { clearAll(); say("anotif"); show("da-dim"); show("da-shade"); },
    qs() { clearAll(); say("aqs"); show("da-dim"); $("#da-shade").classList.add("full"); show("da-shade"); },
    volup() { volume("da-vol", true, "avolup"); },
    voldown() { volume("da-vol", false, "avoldown"); },
    power() { clearAll(); say("apower"); show("da-off"); },
  };

  // Panel buttons. Notifications, Control Center, quick settings, the App
  // Switcher and recents toggle like in the app: a second press closes.
  const OPEN = { switcher: "demo-switcher", notif: "demo-nc", cc: "demo-cc", anotif: "da-shade", aqs: "da-shade", arecents: "da-recents" };
  const isOpen = (g) => {
    if (!OPEN[g] || !$("#" + OPEN[g]).classList.contains("show")) return false;
    if (g === "aqs") return $("#da-shade").classList.contains("full");
    if (g === "anotif") return !$("#da-shade").classList.contains("full");
    return true;
  };
  const press = (os, g) => {
    $$("[data-g]").forEach((x) => x.classList.toggle("on", x.dataset.g === g && x.closest(".gpanel").dataset.os === os));
    (os === "android" ? droid : apple)[g]();
  };
  $$("[data-g]").forEach((b) => b.addEventListener("click", () => {
    app.dataset.touched = "1"; stopAuto();
    const os = b.closest(".gpanel").dataset.os, g = b.dataset.g;
    if (isOpen((os === "android" ? "a" : "") + g)) {
      clearAll(); say("closed"); swipeUp();
      $$("[data-g]").forEach((x) => x.classList.remove("on"));
      return;
    }
    press(os, g);
  }));

  // --- autoplay: every gesture on one device, then the next device ----------
  const SEQ = { apple: ["tap", "home", "cc", "notif", "next", "prev", "switcher", "lock"], android: ["tap", "back", "notif", "qs", "recents", "home", "volup", "power"] };
  const TOUR = ["iphone", "android", "ipad", "atab"];
  let auto = null, step = 0;
  const playStep = () => {
    if (app.classList.contains("studio")) { clearAll(); say("studio"); return; }
    const os = isAndroid() ? "android" : "apple", seq = SEQ[os];
    if (step >= seq.length) { step = 0; selectDevice(TOUR[(TOUR.indexOf(dev) + 1) % TOUR.length], true); return; }
    press(os, seq[step++]);
  };
  const startAuto = () => { stopAuto(); playStep(); auto = setInterval(playStep, 2800); };
  function stopAuto() { if (auto) clearInterval(auto); auto = null; }
  new IntersectionObserver((e) => { if (e[0].isIntersecting) { if (!auto && !app.dataset.touched) startAuto(); } else stopAuto(); }, { threshold: .3 }).observe(app);
  app.addEventListener("pointerdown", () => { app.dataset.touched = "1"; stopAuto(); });

  // --- device + mode switches -------------------------------------------------
  const DEV = {
    iphone: { cls: "phone", name: { id: "iPhone 18 Pro Max", en: "iPhone 18 Pro Max" }, res: "2868×1320" },
    ipad: { cls: "tablet", name: { id: "iPad Pro 13″", en: "iPad Pro 13″" }, res: "2752×2064" },
    android: { cls: "aphone", name: { id: "Android Smartphone", en: "Android Smartphone" }, res: "2400×1080" },
    atab: { cls: "atab", name: { id: "Android Tablet", en: "Android Tablet" }, res: "2560×1600" },
  };
  const MODES = {
    apple: [["apple", "Apple Driver", "Apple Driver"], ["usb", "Studio · Kabel", "Studio · Cable"], ["airplay", "Studio · Wi-Fi", "Studio · Wi-Fi"]],
    android: [["usb", "USB", "USB"], ["wifi", "Wi-Fi", "Wi-Fi"]],
  };
  const modeSeg = $("#seg-mode");
  let mode = "apple";
  const renderTop = () => {
    const d = DEV[dev];
    $("#demo-devname").textContent = `${d.name[lang()]} · ${d.res}`;
    $("#demo-src").textContent = isAndroid() ? (link === "wifi" ? "ANDROID · WI-FI" : "ANDROID · USB")
      : mode === "apple" ? (link === "airplay" ? "APPLE · WI-FI" : "APPLE · USB") : (link === "airplay" ? "STUDIO · AIRPLAY" : "STUDIO · USB");
  };
  const setMode = (m) => {
    $$("button", modeSeg).forEach((x) => x.setAttribute("aria-pressed", String(x.dataset.mode === m)));
    if (isAndroid()) { mode = "android"; link = m; } else { mode = m === "apple" ? "apple" : "studio"; link = m === "airplay" ? "airplay" : "usb"; }
    app.classList.toggle("studio", mode === "studio");
    renderTop();
  };
  const renderModes = () => {
    const os = isAndroid() ? "android" : "apple";
    modeSeg.classList.toggle("android", os === "android");
    modeSeg.innerHTML = MODES[os].map(([m, id, en]) => `<button type="button" data-mode="${m}" aria-pressed="false">${lang() === "en" ? en : id}</button>`).join("");
    $$("button", modeSeg).forEach((b) => b.addEventListener("click", () => {
      setMode(b.dataset.mode); clearAll();
      if (mode === "studio") say("studio"); else if (!auto && !app.dataset.touched) startAuto();
    }));
  };
  function selectDevice(d, fromTour = false) {
    if (!DEV[d]) return;
    const was = isAndroid();
    dev = d;
    $$("#seg-dev button").forEach((x) => x.setAttribute("aria-pressed", String(x.dataset.dev === d)));
    $("#seg-dev").classList.toggle("android", isAndroid());
    device.className = "device " + DEV[d].cls;
    app.classList.toggle("android", isAndroid());
    clearAll(); fillGrid(); setPage(0);
    caption.innerHTML = caption.dataset[lang()] || caption.dataset.id;
    if (was !== isAndroid() || !modeSeg.children.length) { renderModes(); setMode(isAndroid() ? "usb" : "apple"); }
    renderTop(); renderSettings();
    step = 0;
    if (fromTour) playStep();
  }
  $$("#seg-dev button").forEach((b) => b.addEventListener("click", () => { app.dataset.touched = "1"; stopAuto(); selectDevice(b.dataset.dev); }));
  // Hero chips jump to the demo with that device.
  $$("[data-go]").forEach((b) => b.addEventListener("click", () => {
    selectDevice(b.dataset.go);
    $("#preview").scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
    app.dataset.touched = ""; stopAuto(); setTimeout(startAuto, 700);
  }));
  // --- Settings panel: the same cards as the app, and it scrolls -------------------
  const setp = $("#demo-settings");
  const ui = { fit: "contain", rot: "0", glass: true, thick: 30, anim: true, follow: true, audio: "pc", vol: 100, profile: "balanced", res: "1920", fps: "60", rate: "8", conn: "wired" };
  const L = (id, en) => (lang() === "en" ? en : id);
  const ICON = {
    look: "M12 3a9 9 0 1 0 0 18c1 0 1.5-.8 1.5-1.5 0-.9-.7-1.2-.7-2 0-.9.7-1.5 1.6-1.5H17a4 4 0 0 0 4-4c0-5-4-9-9-9z",
    disp: "M4 5h16v11H4zM9 20h6M12 16v4", obs: "M4 7h11v10H4zM15 10l5-3v10l-5-3", audio: "M11 5 6 9H2v6h4l5 4zM15.5 8.5a5 5 0 0 1 0 7",
    ctl: "M3 7h18v10H3zM7 11h.01M11 11h.01M15 11h.01M8 14h8", lock: "M5 11h14v10H5zM8 11V7a4 4 0 0 1 8 0v4",
    plug: "M9 2v6M15 2v6M6 8h12v4a6 6 0 0 1-12 0zM12 18v4", adv: "M4 6h16M4 12h16M4 18h16", vid: "M4 6h12v12H4zM16 10l4-2v8l-4-2", dev: "M7 3h10v18H7z",
  };
  const segOf = (key, opts, cur) => `<div class="sseg" data-k="${key}">${opts.map(([v, l]) => `<button type="button" data-v="${v}" class="${cur === v ? "on" : ""}">${l}</button>`).join("")}</div>`;
  const seg = (key, opts) => segOf(key, opts, String(ui[key]));
  const tog = (key, def) => {
    if (!(key in ui)) ui[key] = def;
    return `<button type="button" class="stg${ui[key] ? " on" : ""}" data-t="${key}" aria-label="toggle"></button>`;
  };
  const row = (label, right, hint = "") => `<div class="srow"><span>${label}${hint ? `<small>${hint}</small>` : ""}</span>${right}</div>`;
  const range = (key) => `<input class="srange" type="range" min="0" max="100" value="${ui[key]}" data-r="${key}" style="--p:${ui[key]}%">`;
  const card = (title, icon, ta, tb, body) =>
    `<section class="scard"><h5><i style="--ta:${ta};--tb:${tb}"><svg viewBox="0 0 24 24"><path d="${ICON[icon]}"/></svg></i>${title}<u>i</u></h5>${body}</section>`;
  const renderSettings = () => {
    const theme = document.documentElement.dataset.pref || "system";
    const and = isAndroid();
    let h = card(L("Tampilan", "Appearance"), "look", "#f472b6", "#db2777",
      segOf("theme", [["light", L("Terang", "Light")], ["dark", L("Gelap", "Dark")], ["system", L("Sistem", "System")]], theme) +
      row("Liquid Glass", tog("glass", true)) + row(L("Ketebalan kaca", "Glass thickness"), `<span class="val">${ui.thick}%</span>`) + range("thick") +
      row(L("Latar animasi", "Animated background"), tog("anim", true)) +
      row(L("Bahasa", "Language"), `<div style="width:160px">${segOf("lang", [["id", "Indonesia"], ["en", "English"]], lang())}</div>`));
    if (and) h += card(L("Video Android", "Android video"), "vid", "#4ade80", "#16a34a",
      seg("profile", [["low-latency", "Low Latency"], ["balanced", "Balanced"], ["high-quality", "High Quality"]]) +
      row(L("Resolusi", "Resolution"), "") + seg("res", [["1280", "720p"], ["1920", "1080p"], ["2560", "1440p"], ["0", "Native"]]) +
      row("FPS", "") + seg("fps", [["30", "30"], ["60", "60"], ["90", "90"], ["120", "120"]]) +
      row("Bitrate", "") + seg("rate", [["4", "4"], ["8", "8"], ["16", "16"], ["24", "24 Mbps"]]) +
      row(L("Sedang dipakai", "In use"), `<span class="val">H.264 · 1080p · 60 fps</span>`));
    h += card(L("Layar & rasio", "Display & aspect"), "disp", "#a78bfa", "#7c3aed",
      seg("fit", [["contain", "Auto-Fit"], ["cover", "Edge-to-Edge"], ["fill", "Stretch"]]) +
      row(L("Jendela ikut ukuran layar", "Window follows the screen"), tog("follow", true), L("Potret / lanskap otomatis", "Portrait / landscape automatically")) +
      row(L("Rotasi", "Rotation"), "") + seg("rot", [["auto", "Auto"], ["0", "0°"], ["90", "90°"], ["180", "180°"], ["270", "270°"]]));
    h += card(L("Rekam / live di OBS", "Record / stream in OBS"), "obs", "#f87171", "#dc2626",
      `<p>${L("Di OBS, tambah sumber Game Capture → Mode: tangkap jendela tertentu → pilih [ZenLink.exe]: ZenLink. Lalu pilih sumbernya dan tekan Ctrl+F (Fit to screen): gambar potret/lanskap otomatis pas di tengah.",
        "In OBS, add a Game Capture source → Mode: capture a specific window → pick [ZenLink.exe]: ZenLink. Then select the source and press Ctrl+F (Fit to screen): portrait or landscape lands centred automatically.")}</p>`);
    h += card(L("Audio perangkat → PC", "Device audio → PC"), "audio", "#fb923c", "#ea580c",
      seg("audio", [["pc", L("Di PC ini", "On this PC")], ["device", L("Hanya perangkat", "Device only")]]) +
      row(L("Volume PC", "PC volume"), `<span class="val">${ui.vol}%</span>`) + range("vol") + row("Lip sync", tog("lips", true)));
    h += card(L("Kontrol", "Control"), "ctl", "#38bdf8", "#0284c7",
      row(L("Sentuh dengan mouse", "Touch with the mouse"), tog("mouse", true)) + row(L("Keyboard PC ke perangkat", "PC keyboard to the device"), tog("kbd", true)) +
      (and ? "" : row(L("Gesture (scroll, tombol Back mouse)", "Gestures (scroll, mouse Back button)"), tog("gest", true))));
    if (!and) h += card(L("Buka kunci otomatis", "Auto unlock"), "lock", "#a78bfa", "#7c3aed",
      `<p style="color:#22c55e;font-weight:600">${L("Kode tersimpan di PC ini.", "A passcode is saved on this PC.")}</p>` +
      `<div class="srow"><span class="sfield">••••••</span><button type="button" class="sbtn">${L("Hapus", "Remove")}</button></div>` +
      row(L("Buka kunci saat mirroring mulai", "Unlock when mirroring starts"), tog("unlock", true), L("Juga Alt+U kapan saja", "Also Alt+U at any time")));
    h += card(L("Jenis perangkat", "Device type"), "dev", "#f472b6", "#db2777",
      row(L("Ingat jenis perangkat", "Remember device type"), tog("remember", true)) +
      `<div class="sbtns"><button type="button" class="sbtn">${L("Ganti jenis perangkat", "Change device type")}</button></div>`);
    if (!and) h += card(L("Koneksi & driver", "Connection & driver"), "plug", "#38bdf8", "#0284c7",
      row(L("Sambungan", "Connection"), "") + seg("conn", [["wired", L("Kabel", "Cable")], ["wireless", "Wi-Fi"]]) +
      `<div class="sbtns"><button type="button" class="sbtn">${L("Ganti mode", "Change mode")}</button><button type="button" class="sbtn">${L("Periksa driver", "Check drivers")}</button><button type="button" class="sbtn">${L("Atur Wi-Fi", "Set up Wi-Fi")}</button></div>`);
    h += card(L("Lanjutan", "Advanced"), "adv", "#94a3b8", "#475569",
      row(L("Mulai bersama Windows", "Start with Windows"), tog("autostart", false)) + row(L("Tutup ke tray", "Close to the tray"), tog("tray", false)) +
      row(L("Selalu di atas", "Always on top"), tog("top", false)) + row(L("Statistik di header", "Stats in the header"), tog("stats", true)));
    const keep = setp.querySelector(".sc") ? setp.querySelector(".sc").scrollTop : 0;
    setp.innerHTML = `<div class="hd"><img src="logo.png" alt=""><div><b>ZenLink</b><small>${L("Pengaturan & kontrol", "Settings & control")}</small></div><button type="button" data-close aria-label="Close"><svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6 6 18"/></svg></button></div><div class="sc">${h}</div>`;
    setp.querySelector(".sc").scrollTop = keep;
  };
  const applyView = () => {
    const r = ui.rot === "auto" ? 0 : +ui.rot, side = r % 180 !== 0;
    const fit = ui.fit === "cover" ? " scale(1.22)" : ui.fit === "fill" ? " scale(1.18, 1)" : "";
    device.style.transform = `rotate(${r}deg)${side ? " scale(.62)" : ""}${fit}`;
    app.classList.toggle("no-glass", !ui.glass);
    app.classList.toggle("still", !ui.anim);
    $("#st-audio").closest(".st").style.visibility = ui.stats === false ? "hidden" : "";
  };
  setp.addEventListener("click", (e) => {
    const b = e.target.closest("button"); if (!b) return;
    if (b.hasAttribute("data-close")) { app.classList.remove("set-open"); return; }
    const sg = b.closest(".sseg");
    if (sg) {
      const k = sg.dataset.k, v = b.dataset.v;
      if (k === "lang") { const lb = document.querySelector(`.lang button[data-lang="${v}"]`); if (lb) lb.click(); return; }
      if (k === "theme") { if (window.zlTheme) window.zlTheme(v); } else ui[k] = v;
      sg.querySelectorAll("button").forEach((x) => x.classList.toggle("on", x === b));
      applyView(); return;
    }
    if (b.dataset.t) { ui[b.dataset.t] = !b.classList.contains("on"); b.classList.toggle("on"); applyView(); }
  });
  setp.addEventListener("input", (e) => {
    const r = e.target.closest("[data-r]"); if (!r) return;
    ui[r.dataset.r] = +r.value; r.style.setProperty("--p", r.value + "%");
    const val = r.previousElementSibling && r.previousElementSibling.querySelector(".val");
    if (val) val.textContent = r.value + "%";
    if (r.dataset.r === "thick") app.style.setProperty("--thick", r.value / 100);
  });
  $("#demo-gear").addEventListener("click", () => { app.classList.toggle("set-open"); renderSettings(); });
  document.addEventListener("zenlink:theme", () => { if (setp.innerHTML) renderSettings(); });
  if (innerWidth > 1100) app.classList.add("set-open");
  // A click on the picture is a tap, like the mouse in the app.
  screen.addEventListener("click", () => { app.dataset.touched = "1"; stopAuto(); press(isAndroid() ? "android" : "apple", "tap"); });

  selectDevice("iphone");
  document.addEventListener("zenlink:lang", () => {
    tick();
    const keep = $("button[aria-pressed='true']", modeSeg)?.dataset.mode;
    renderModes(); if (keep) setMode(keep);
    renderTop(); renderSettings();
  });
})();
