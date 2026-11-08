// Frameless window chrome: window buttons, edge resize, real full screen with
// an auto-hiding bar. Dragging is pywebview's own (.pywebview-drag-region).
"use strict";

(() => {
  const api = () => window.pywebview && window.pywebview.api;
  const body = document.body;
  let hideTimer = 0;

  function onReady() {
    body.classList.add("pywebview");
    syncMaximized();
  }
  if (api()) onReady();
  else window.addEventListener("pywebviewready", onReady);

  async function syncMaximized() {
    const a = api();
    if (!a) return;
    const max = Boolean(await a.is_maximized());
    body.classList.toggle("maximized", max);
    const btn = document.querySelector('[data-win="maximize"]');
    btn.title = max ? "Restore" : "Maximize";
    btn.setAttribute("aria-label", btn.title);
    btn.innerHTML = max
      ? '<svg viewBox="0 0 10 10" aria-hidden="true"><rect x="1" y="3" width="6" height="6" rx="1"/><path d="M3 3V1.8A.8.8 0 0 1 3.8 1H8.2a.8.8 0 0 1 .8.8v4.4a.8.8 0 0 1-.8.8H7"/></svg>'
      : '<svg viewBox="0 0 10 10" aria-hidden="true"><rect x="1" y="1" width="8" height="8" rx="1"/></svg>';
  }

  document.querySelectorAll("[data-win]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const a = api();
      if (!a) return;
      if (btn.dataset.win === "minimize") a.minimize();
      else if (btn.dataset.win === "maximize") { await a.toggle_maximize(); syncMaximized(); }
      else a.close();
    });
  });

  document.querySelectorAll(".drag-spacer, .brand").forEach((el) =>
    el.addEventListener("mousedown", async (e) => {
      // Two presses within 350 ms on the title bar = maximize / restore
      // (pywebview's native drag swallows the dblclick event).
      if (e.button !== 0) return;
      const now = performance.now();
      if (now - (el._lastDown || 0) < 350) {
        el._lastDown = 0;
        await api()?.toggle_maximize();
        syncMaximized();
      } else {
        el._lastDown = now;
      }
    })
  );

  document.querySelectorAll(".grips i").forEach((grip) => {
    grip.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      api()?.start_resize(grip.dataset.edge);
    });
  });

  // ---- full screen: the whole window covers the monitor; a bar appears on
  // mouse movement and hides again after 2.5 s of stillness.
  function showChrome() {
    body.classList.add("chrome-visible");
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (document.querySelector(".fs-bar:hover")) showChrome();
      else body.classList.remove("chrome-visible");
    }, 2500);
  }

  async function setFullscreen(on) {
    body.classList.toggle("fullscreen", on);
    const a = api();
    if (a) await a.set_fullscreen(on);
    else if (on) document.documentElement.requestFullscreen?.().catch(() => {});
    else if (document.fullscreenElement) document.exitFullscreen?.().catch(() => {});
    if (on) showChrome();
    window.dispatchEvent(new Event("resize"));
  }
  window.ZenLinkWindow = {
    toggleFullscreen: () => setFullscreen(!body.classList.contains("fullscreen")),
    exitFullscreen: () => body.classList.contains("fullscreen") && setFullscreen(false),
    setOnTop: (on) => api()?.set_on_top(Boolean(on)),
    // Resize the window so the picture fills it with the device's shape.
    fitAspect: (w, h) => {
      if (!w || !h || body.classList.contains("fullscreen") || body.classList.contains("maximized")) return;
      const stage = document.getElementById("stage");
      const chromeH = window.innerHeight - stage.clientHeight;
      api()?.fit_aspect(w / h, chromeH);
    },
  };

  // The bar shows only when the mouse goes to the top of the screen, and
  // stays while the mouse is on it.
  document.addEventListener("mousemove", (e) => {
    if (!body.classList.contains("fullscreen")) return;
    if (e.clientY <= 90 || e.target.closest?.(".fs-bar")) showChrome();
  });
  document.getElementById("fs-exit").addEventListener("click", () => setFullscreen(false));
  window.addEventListener("resize", syncMaximized);
})();
