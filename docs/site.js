// ZenLink site: language, theme, glass highlights, reveal, live release data.
(() => {
  const root = document.documentElement;
  const calm = matchMedia("(prefers-reduced-motion: reduce)").matches;

  // --- theme: system (default) → light → dark, remembered per browser -------
  const TKEY = "zenlink.site.theme";
  const sysLight = matchMedia("(prefers-color-scheme: light)");
  const themeMeta = document.querySelector('meta[name="theme-color"]');
  const applyTheme = (pref) => {
    root.dataset.pref = pref;
    root.dataset.theme = pref === "system" ? (sysLight.matches ? "light" : "dark") : pref;
    document.dispatchEvent(new Event("zenlink:theme"));
    if (themeMeta) themeMeta.content = root.dataset.theme === "light" ? "#eef0f7" : "#06060a";
    const btn = document.getElementById("theme-btn");
    if (btn) {
      const en = root.lang === "en";
      const name = { system: en ? "Theme: follow the device" : "Tema: ikut perangkat", light: en ? "Theme: light" : "Tema: terang", dark: en ? "Theme: dark" : "Tema: gelap" }[pref];
      btn.title = name; btn.setAttribute("aria-label", name);
    }
  };
  let pref = root.dataset.pref || "system";
  applyTheme(pref);
  sysLight.addEventListener("change", () => { if (pref === "system") applyTheme(pref); });
  // The demo's Settings panel sets the theme too.
  window.zlTheme = (p) => {
    pref = p;
    try { localStorage.setItem(TKEY, pref); } catch (_) {}
    applyTheme(pref);
  };
  document.getElementById("theme-btn").addEventListener("click", () => {
    pref = { system: "light", light: "dark", dark: "system" }[pref] || "system";
    try { localStorage.setItem(TKEY, pref); } catch (_) {}
    applyTheme(pref);
  });

  // --- language: Indonesian by default, remembered per browser --------------
  const KEY = "zenlink.site.lang";
  const shownVal = new Map();
  const fmt = (n) => n.toLocaleString(root.lang === "en" ? "en-US" : "id-ID");
  function setLang(lang) {
    root.lang = lang;
    document.querySelectorAll("[data-id]").forEach((el) => { el.innerHTML = el.dataset[lang] ?? el.dataset.id; });
    document.querySelectorAll("[data-ph-id]").forEach((el) => { el.placeholder = lang === "en" ? el.dataset.phEn : el.dataset.phId; });
    document.querySelectorAll(".lang button").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.lang === lang)));
    try { localStorage.setItem(KEY, lang); } catch (_) {}
    document.querySelectorAll(".dl-total").forEach((el) => { if (shownVal.has(el)) el.textContent = fmt(shownVal.get(el)); });
    applyTheme(pref);
    document.dispatchEvent(new Event("zenlink:lang"));
  }
  document.querySelectorAll(".lang button").forEach((b) => b.addEventListener("click", () => setLang(b.dataset.lang)));
  let saved = "id";
  try { saved = localStorage.getItem(KEY) || "id"; } catch (_) {}
  setLang(saved);

  // --- glass: the highlight follows the pointer -------------------------------
  let pending = null, queued = false;
  document.addEventListener("pointermove", (e) => {
    if (calm || e.pointerType === "touch") return;
    pending = e;
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      const ev = pending; pending = null; queued = false;
      const el = ev.target.closest && ev.target.closest(".glass");
      if (!el) return;
      const r = el.getBoundingClientRect();
      el.style.setProperty("--mx", `${ev.clientX - r.left}px`);
      el.style.setProperty("--my", `${ev.clientY - r.top}px`);
    });
  }, { passive: true });

  // --- hero stage: each device steps forward in turn and plays a short demo ---
  const cluster = document.getElementById("cluster");
  if (cluster) {
    const ORDER = ["iphone", "android", "ipad", "atab"];
    const INFO = {
      iphone: { name: ["iPhone", "iPhone"], sub: ["iOS · Apple Driver & Studio", "iOS · Apple Driver & Studio"], android: false },
      android: { name: ["Android Smartphone", "Android Smartphone"], sub: ["Android 5.0+ · USB & Wi-Fi", "Android 5.0+ · USB & Wi-Fi"], android: true },
      ipad: { name: ["iPad", "iPad"], sub: ["iPadOS · kabel & AirPlay", "iPadOS · cable & AirPlay"], android: false },
      atab: { name: ["Android Tablet", "Android Tablet"], sub: ["Android · layar lebar", "Android · big screen"], android: true },
    };
    const devs = Object.fromEntries(Array.from(cluster.querySelectorAll(".d")).map((d) => [d.dataset.k, d]));
    const tag = cluster.querySelector(".stagetag"), pips = cluster.querySelectorAll(".pips i");
    let idx = 0, lx = 0, ly = 0, timer = 0, visible = true;
    const layout = () => {
      const cw = cluster.offsetWidth, ch = cluster.offsetHeight, key = ORDER[idx];
      Object.entries(devs).forEach(([k, d], i) => {
        const dx = d.offsetLeft + d.offsetWidth / 2 - cw / 2, dy = d.offsetTop + d.offsetHeight / 2 - ch / 2 + 10;
        const on = k === key, tab = k === "ipad" || k === "atab";
        const s = on ? (tab ? 1.1 : 1.16) : .86;
        const tx = on ? -dx : dx * .22, ty = on ? -dy : dy * .1;
        const lean = calm ? "" : ` rotateY(${lx * (on ? 10 : 6 + i)}deg) rotateX(${-ly * (on ? 7 : 4)}deg)`;
        d.style.transform = `translate(${tx + lx * (on ? 10 : 18)}px, ${ty + ly * 8}px) scale(${s})${lean}`;
        d.classList.toggle("on", on); d.classList.toggle("away", !on);
      });
    };
    const label = () => {
      const k = ORDER[idx], info = INFO[k], l = root.lang === "en" ? 1 : 0;
      tag.querySelector("b").textContent = info.name[l];
      tag.querySelector("small").textContent = info.sub[l];
      tag.classList.toggle("android", info.android);
      pips.forEach((p, i) => p.classList.toggle("on", i === idx));
    };
    const show = (i) => {
      idx = i;
      Object.values(devs).forEach((d) => d.classList.remove("play"));
      tag.classList.add("swap");
      setTimeout(() => { label(); tag.classList.remove("swap"); }, 350);
      layout();
      if (!calm) { const d = devs[ORDER[idx]]; void d.offsetWidth; setTimeout(() => d.classList.add("play"), 650); }
    };
    const next = () => { if (visible && !document.hidden) show((idx + 1) % ORDER.length); };
    label(); show(0);
    if (!calm) timer = setInterval(next, 4600);
    new IntersectionObserver((e) => { visible = e[0].isIntersecting; }).observe(cluster);
    window.addEventListener("resize", layout, { passive: true });
    document.addEventListener("zenlink:lang", label);
    if (!calm) {
      let raf = 0;
      window.addEventListener("pointermove", (e) => {
        if (raf || e.pointerType === "touch") return;
        raf = requestAnimationFrame(() => { raf = 0; lx = e.clientX / innerWidth - .5; ly = e.clientY / innerHeight - .5; layout(); });
      }, { passive: true });
    }
    void timer;
  }

  // --- reveal on scroll ----------------------------------------------------------
  const io = new IntersectionObserver((entries) => entries.forEach((e) => {
    if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); }
  }), { threshold: .12 });
  document.querySelectorAll(".reveal").forEach((el, i) => { el.style.transitionDelay = `${(i % 3) * 70}ms`; io.observe(el); });

  // --- download total: counts up the first time it is on screen -------------------
  let lastTotal = null;
  function animateTo(el, to) {
    const from = shownVal.has(el) ? shownVal.get(el) : 0;
    shownVal.set(el, to);
    if (calm || from === to) { el.textContent = fmt(to); return; }
    const t0 = performance.now(), dur = from === 0 ? 1800 : 900;
    const step = (t) => {
      if (shownVal.get(el) !== to) return; // a newer number took over
      const k = Math.min(1, (t - t0) / dur), ease = 1 - Math.pow(1 - k, 3);
      el.textContent = fmt(Math.round(from + (to - from) * ease));
      if (k < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
    // Background tabs pause animation frames: land on the number anyway.
    setTimeout(() => { if (shownVal.get(el) === to) el.textContent = fmt(to); }, dur + 150);
  }
  const onScreen = (el) => { const r = el.getBoundingClientRect(); return r.bottom > 0 && r.top < innerHeight; };
  const countIO = new IntersectionObserver((entries) => entries.forEach((e) => {
    if (!e.isIntersecting) return;
    e.target.dataset.seen = "1"; countIO.unobserve(e.target);
    if (lastTotal !== null) animateTo(e.target, lastTotal);
  }), { threshold: .6 });
  document.querySelectorAll(".dl-total").forEach((el) => countIO.observe(el));
  function showTotal(n) {
    lastTotal = n;
    document.querySelectorAll(".dl-total").forEach((el) => {
      if (!el.dataset.seen && onScreen(el)) { el.dataset.seen = "1"; countIO.unobserve(el); }
      if (el.dataset.seen) animateTo(el, n);
    });
  }

  // --- feedback opens a prefilled GitHub issue the visitor reviews -----------------
  document.getElementById("fb").addEventListener("submit", (e) => {
    e.preventDefault();
    const text = document.getElementById("fb-text").value.trim();
    if (!text) return;
    const title = text.split("\n")[0].slice(0, 70);
    const url = `https://github.com/zenvyzenshiro/zenlink/issues/new?title=${encodeURIComponent(title)}&body=${encodeURIComponent(text)}`;
    window.open(url, "_blank", "noopener");
  });

  // --- live version, file links, SHA-256 and download total from GitHub ------------
  // Refreshed every 5 minutes and when the tab comes back (GitHub allows 60
  // unauthenticated requests per hour).
  const repo = "zenvyzenshiro/zenlink";
  let loadedAt = 0;
  function load() {
    loadedAt = Date.now();
    return fetch(`https://api.github.com/repos/${repo}/releases?per_page=100`)
      .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
      .then((releases) => {
        let total = 0;
        releases.forEach((rel) => rel.assets.forEach((a) => { total += a.download_count; }));
        showTotal(total);
        const latest = releases.find((r) => !r.draft && !r.prerelease);
        if (!latest) return;
        document.querySelectorAll(".ver").forEach((el) => { el.textContent = latest.tag_name.replace(/^v/, ""); });
        const pick = (re) => latest.assets.find((a) => re.test(a.name));
        const setup = pick(/Setup\.exe$/i), portable = pick(/Portable\.exe$/i), source = pick(/Source\.zip$/i);
        // Sizes: the latest release next to v2.2.31, struck through.
        const mb = (n) => `${(n / 1048576).toFixed(1)} MB`;
        const old = releases.find((r) => r.tag_name === "v2.2.31");
        const oldPick = (re) => old && old.assets.find((a) => re.test(a.name));
        const compare = (now, before, key) => {
          if (!now) return;
          document.querySelectorAll(key === "setup" ? ".size" : ".size-p").forEach((el) => { el.textContent = mb(now.size); });
          if (!before) return;
          document.querySelectorAll(`.old-${key}`).forEach((el) => { el.textContent = mb(before.size); });
          const save = Math.round((1 - now.size / before.size) * 100);
          document.querySelectorAll(`.save-${key}`).forEach((el) => { el.textContent = save > 0 ? `−${save}%` : `+${-save}%`; });
          document.querySelectorAll(`[data-w="${key}"]`).forEach((el) => { el.style.setProperty("--w", `${Math.min(100, Math.round((now.size / before.size) * 100))}%`); });
        };
        compare(setup, oldPick(/Setup\.exe$/i), "setup");
        compare(portable, oldPick(/Portable\.exe$/i), "portable");
        if (setup) {
          document.getElementById("f-setup").href = setup.browser_download_url;
          if (/^sha256:/.test(setup.digest || "")) {
            document.getElementById("hash-name").textContent = setup.name;
            document.getElementById("hash").textContent = setup.digest.slice(7);
            document.getElementById("hash-wrap").hidden = false;
          }
        }
        if (portable) document.getElementById("f-portable").href = portable.browser_download_url;
        if (source) document.getElementById("f-source").href = source.browser_download_url;
      })
      .catch(() => {});
  }
  load();
  setInterval(() => { if (!document.hidden) load(); }, 5 * 60 * 1000);
  document.addEventListener("visibilitychange", () => { if (!document.hidden && Date.now() - loadedAt > 60 * 1000) load(); });
})();
