// ZenLink live demo: gestures on a mirrored iPhone 18 Pro Max / iPad Air.
(() => {
  const $ = (s, r = document) => r.querySelector(s);
  const app = $("#demo-app"), device = $("#demo-device"), screen = $("#demo-screen");
  if (!app) return;
  const lang = () => (document.documentElement.lang === "en" ? "en" : "id");
  const T = {
    home: { id: "Home — usap ke atas dari bawah", en: "Home — swipe up from the bottom" },
    switcher: { id: "App Switcher — Home dua kali", en: "App Switcher — Home twice" },
    notif: { id: "Notifikasi — usap dari kiri atas", en: "Notifications — swipe from the top left" },
    cc: { id: "Pusat Kontrol — usap dari kanan atas", en: "Control Center — swipe from the top right" },
    next: { id: "Usap ← halaman berikutnya", en: "Swipe ← next page" },
    prev: { id: "Usap → halaman sebelumnya", en: "Swipe → previous page" },
    volup: { id: "Volume +", en: "Volume +" },
    voldown: { id: "Volume −", en: "Volume −" },
    lock: { id: "Kunci layar · Alt+L", en: "Lock screen · Alt+L" },
    tap: { id: "Klik mouse = sentuhan di layar", en: "Mouse click = a tap on the screen" },
    type: { id: "Ketik dengan keyboard PC", en: "Type with your PC keyboard" },
    studio: { id: "Gambar & suara saja — kualitas untuk live", en: "Picture & sound only — quality for live" },
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

  // --- clock -----------------------------------------------------------
  const tick = () => {
    const d = new Date();
    const t = d.toLocaleTimeString(lang() === "en" ? "en-US" : "id-ID", { hour: "2-digit", minute: "2-digit", hour12: false }).replace(".", ":");
    document.querySelectorAll("[data-clock]").forEach((el) => { el.textContent = t; });
    $("#demo-date").textContent = d.toLocaleDateString(lang() === "en" ? "en-US" : "id-ID", { weekday: "long", day: "numeric", month: "long" });
  };
  tick(); setInterval(tick, 15000);

  // --- live stats in the header ------------------------------------------
  let link = "usb";
  setInterval(() => {
    const fps = Math.round(57 + Math.random() * 3);
    const video = link === "airplay" ? Math.round(13 + Math.random() * 6) : Math.round(1 + Math.random() * 2);
    const audio = link === "airplay" ? Math.round(140 + Math.random() * 25) : Math.round(22 + Math.random() * 3);
    $("#st-fps").textContent = fps; $("#st-video").textContent = video; $("#st-audio").textContent = audio;
    $("#st-audio").parentElement.classList.toggle("warn", audio >= 150);
  }, 900);

  // --- gestures --------------------------------------------------------
  const ov = (id) => $("#demo-" + id);
  const caption = $("#demo-caption");
  const say = (key) => { caption.innerHTML = T[key][lang()]; };
  const cursor = $("#demo-cursor"), trail = $("#demo-trail");
  let page = 0, busy = false, timers = [];
  const later = (fn, ms) => timers.push(setTimeout(fn, ms));
  const clearAll = () => {
    timers.forEach(clearTimeout); timers = [];
    ["cc", "nc", "switcher", "vol", "lock", "dim"].forEach((k) => ov(k).classList.remove("show"));
    $("#demo-appwin").classList.remove("open");
    $("#demo-home").style.transform = ""; cursor.classList.remove("show", "tapping"); trail.classList.remove("go");
    $("#demo-wall").style.transform = "";
  };
  const setPage = (p) => {
    page = p; $("#demo-pages").style.transform = `translateX(${-50 * p}%)`;
    document.querySelectorAll("#demo-dots i").forEach((d, i) => d.classList.toggle("on", i === p));
  };
  const moveCursor = (xPct, yPct) => { cursor.style.left = xPct + "%"; cursor.style.top = yPct + "%"; };
  const swipeUp = () => { trail.classList.add("go"); later(() => trail.classList.remove("go"), 520); };

  const run = {
    tap() {
      clearAll(); say("tap");
      const icons = document.querySelectorAll("#demo-page1 .icon i");
      const target = icons[12] || icons[0];
      const sr = screen.getBoundingClientRect(), r = target.getBoundingClientRect();
      const x = ((r.left + r.width / 2 - sr.left) / sr.width) * 100, y = ((r.top + r.height / 2 - sr.top) / sr.height) * 100;
      setPage(0); moveCursor(60, 85); cursor.classList.add("show");
      later(() => moveCursor(x, y), 80);
      later(() => { cursor.classList.add("tapping"); }, 820);
      later(() => {
        const w = $("#demo-appwin"); w.style.setProperty("--ox", x + "%"); w.style.setProperty("--oy", y + "%"); w.classList.add("open");
        $("#demo-home").style.transform = "scale(.92)"; cursor.classList.remove("tapping");
      }, 1000);
      later(() => cursor.classList.remove("show"), 1900);
    },
    home() {
      const w = $("#demo-appwin");
      if (!w.classList.contains("open")) { run.tap(); later(() => run.home(), 2300); return; }
      say("home"); swipeUp();
      later(() => { w.classList.remove("open"); $("#demo-home").style.transform = ""; }, 260);
    },
    switcher() { clearAll(); say("switcher"); swipeUp(); later(() => ov("switcher").classList.add("show"), 220); },
    notif() { clearAll(); say("notif"); ov("nc").classList.add("show"); },
    cc() { clearAll(); say("cc"); ov("dim").classList.add("show"); ov("cc").classList.add("show"); },
    next() { clearAll(); say("next"); setPage(1); },
    prev() { clearAll(); say("prev"); setPage(0); },
    volume(up) {
      clearAll(); say(up ? "volup" : "voldown");
      const v = ov("vol"); const cur = parseInt(getComputedStyle(v).getPropertyValue("--v")) || 55;
      v.style.setProperty("--v", Math.max(8, Math.min(100, cur + (up ? 18 : -18))) + "%"); v.classList.add("show");
      later(() => v.classList.remove("show"), 1400);
    },
    lock() { clearAll(); say("lock"); ov("lock").classList.add("show"); },
  };

  // panel buttons
  document.querySelectorAll("[data-g]").forEach((b) => b.addEventListener("click", () => {
    stopAuto();
    const g = b.dataset.g;
    document.querySelectorAll("[data-g]").forEach((x) => x.classList.toggle("on", x === b));
    if (g === "volup") return run.volume(true);
    if (g === "voldown") return run.volume(false);
    run[g]();
  }));

  // autoplay until the visitor clicks
  const SEQ = ["tap", "home", "cc", "notif", "next", "prev", "switcher", "volup", "lock"];
  let auto = null, step = 0;
  const playStep = () => {
    if (app.classList.contains("studio")) { clearAll(); say("studio"); return; }
    const g = SEQ[step++ % SEQ.length];
    document.querySelectorAll("[data-g]").forEach((x) => x.classList.toggle("on", x.dataset.g === g));
    if (g === "volup") run.volume(true); else run[g]();
  };
  const startAuto = () => { stopAuto(); playStep(); auto = setInterval(playStep, 2800); };
  function stopAuto() { if (auto) clearInterval(auto); auto = null; }
  new IntersectionObserver((e) => { if (e[0].isIntersecting) { if (!auto && !app.dataset.touched) startAuto(); } else stopAuto(); }, { threshold: .3 }).observe(app);
  app.addEventListener("pointerdown", () => { app.dataset.touched = "1"; });

  // --- device + mode switches -----------------------------------------
  const DEV = {
    phone: { id: "iPhone 18 Pro Max", res: "2868×1320" },
    tablet: { id: "iPad Air", res: "2360×1640" },
  };
  let mode = "apple";
  const renderTop = () => {
    const d = device.classList.contains("tablet") ? DEV.tablet : DEV.phone;
    $("#demo-devname").textContent = `${d.id} · ${d.res}`;
    $("#demo-src").textContent = mode === "apple" ? (link === "airplay" ? "APPLE · WI-FI" : "APPLE · USB") : (link === "airplay" ? "STUDIO · AIRPLAY" : "STUDIO · USB");
  };
  document.querySelectorAll("[data-dev]").forEach((b) => b.addEventListener("click", () => {
    document.querySelectorAll("[data-dev]").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
    device.classList.toggle("phone", b.dataset.dev === "phone");
    device.classList.toggle("tablet", b.dataset.dev === "tablet");
    clearAll(); renderTop();
  }));
  document.querySelectorAll("[data-mode]").forEach((b) => b.addEventListener("click", () => {
    document.querySelectorAll("[data-mode]").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
    mode = b.dataset.mode === "apple" ? "apple" : "studio";
    link = b.dataset.mode === "airplay" ? "airplay" : "usb";
    app.classList.toggle("studio", mode === "studio");
    clearAll(); renderTop();
    if (mode === "studio") say("studio"); else if (!auto) startAuto();
  }));
  renderTop();
  document.addEventListener("zenlink:lang", () => { tick(); });
})();
