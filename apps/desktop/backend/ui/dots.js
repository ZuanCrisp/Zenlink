// Animated dot field behind the stage: a slow light wave travels outward
// from the centre and dots near the cursor brighten. Paused while live,
// while the window is hidden, and for reduced-motion users.
"use strict";

(() => {
  const canvas = document.getElementById("dots");
  const stage = document.getElementById("stage");
  if (!canvas || !stage) return;
  const ctx = canvas.getContext("2d");
  const GAP = 22;
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
  let w = 0, h = 0, dpr = 1;
  let mouse = { x: -1e4, y: -1e4 };
  let raf = 0;
  let last = 0;

  function resize() {
    dpr = window.devicePixelRatio || 1;
    w = stage.clientWidth;
    h = stage.clientHeight;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    draw(performance.now());
  }

  function colors() {
    const css = getComputedStyle(document.documentElement);
    return { base: css.getPropertyValue("--dot").trim(), hot: css.getPropertyValue("--dot-hot").trim() };
  }

  function draw(t) {
    const { base, hot } = colors();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const cx = w / 2, cy = h / 2;
    const maxR = Math.hypot(cx, cy);
    const wave = ((t / 1000) * 120) % (maxR + 240) - 120; // radius of the travelling ring
    const ox = (w % GAP) / 2, oy = (h % GAP) / 2;
    for (let y = oy; y < h; y += GAP) {
      for (let x = ox; x < w; x += GAP) {
        const d = Math.hypot(x - cx, y - cy);
        const ring = Math.max(0, 1 - Math.abs(d - wave) / 90);
        const near = Math.max(0, 1 - Math.hypot(x - mouse.x, y - mouse.y) / 140);
        const k = Math.min(1, ring * 0.55 + near * 0.9);
        ctx.globalAlpha = 0.75 + k * 0.25;
        ctx.fillStyle = k > 0.05 ? hot : base;
        const r = 1 + k * 0.9;
        ctx.beginPath();
        ctx.arc(x, y, r, 0, Math.PI * 2);
        ctx.fill();
      }
    }
    ctx.globalAlpha = 1;
  }

  function frame(t) {
    raf = 0;
    if (t - last >= 33) {  // ~30 fps is plenty for a background
      last = t;
      draw(t);
    }
    schedule();
  }

  function running() {
    return !document.hidden && !reduced.matches && !stage.classList.contains("live");
  }

  function schedule() {
    if (!raf && running()) raf = requestAnimationFrame(frame);
  }

  stage.addEventListener("pointermove", (e) => {
    const r = stage.getBoundingClientRect();
    mouse = { x: e.clientX - r.left, y: e.clientY - r.top };
  });
  stage.addEventListener("pointerleave", () => { mouse = { x: -1e4, y: -1e4 }; });
  document.addEventListener("visibilitychange", schedule);
  new MutationObserver(schedule).observe(stage, { attributes: true, attributeFilter: ["class"] });
  new MutationObserver(() => draw(performance.now())).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  new ResizeObserver(resize).observe(stage);
  resize();
  schedule();
})();
