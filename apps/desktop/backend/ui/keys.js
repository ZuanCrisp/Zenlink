// Forwards the PC keyboard to the iPhone as a hardware keyboard. Keys are
// sent by physical position (KeyboardEvent.code -> HID usage); the iPhone's
// own keyboard layout decides which character is typed.
(function () {
  const CODE_TO_HID = (() => {
    const m = {};
    for (let i = 0; i < 26; i++) m[`Key${String.fromCharCode(65 + i)}`] = 0x04 + i;
    for (let i = 1; i <= 9; i++) m[`Digit${i}`] = 0x1d + i;
    m.Digit0 = 0x27;
    Object.assign(m, {
      Enter: 0x28, NumpadEnter: 0x28, Escape: 0x29, Backspace: 0x2a, Tab: 0x2b, Space: 0x2c,
      Minus: 0x2d, Equal: 0x2e, BracketLeft: 0x2f, BracketRight: 0x30, Backslash: 0x31,
      Semicolon: 0x33, Quote: 0x34, Backquote: 0x35, Comma: 0x36, Period: 0x37, Slash: 0x38,
      IntlBackslash: 0x64, CapsLock: 0x39, Delete: 0x4c, Home: 0x4a, End: 0x4d,
      PageUp: 0x4b, PageDown: 0x4e,
      ArrowRight: 0x4f, ArrowLeft: 0x50, ArrowDown: 0x51, ArrowUp: 0x52,
      ShiftLeft: 0xe1, ShiftRight: 0xe5, AltLeft: 0xe2, AltRight: 0xe6,
    });
    return m;
  })();

  async function runAction(action) {
    let body = { action };
    if (action === "paste-pc") {
      let text = "";
      try { text = await navigator.clipboard.readText(); } catch (_) {}
      if (!text) { window.ZenLinkToast?.("The PC clipboard has no text."); return; }
      body.text = text;
    }
    try {
      const resp = await fetch("/api/keyboard/action", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const data = await resp.json().catch(() => ({}));
      if (!resp.ok || data.ok === false) window.ZenLinkToast?.(data.message || "That shortcut needs Apple Driver mode and a live cast.");
      else if (action === "paste-pc") window.ZenLinkToast?.(`Typed ${body.text.length} characters on the device.`);
    } catch (err) {
      window.ZenLinkToast?.(String(err));
    }
  }

  function isTextField(el) {
    return !!el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName));
  }

  function attach({ post, enabled }) {
    const pressed = new Set();
    let chain = Promise.resolve();
    let ignoreKeyUpUntil = 0;

    const flush = () => {
      const snapshot = [...pressed];
      chain = chain.then(() => post({ usages: snapshot })).catch(() => {});
    };

    const releaseAll = () => {
      if (!pressed.size) return;
      pressed.clear();
      flush();
    };

    window.addEventListener(
      "keydown",
      async (e) => {
        if (!enabled() || isTextField(e.target)) return;

        // ZenLink shortcuts (each is one request to /api/keyboard/action).
        const ctrl = (e.ctrlKey || e.metaKey) && !e.altKey;
        const alt = e.altKey && !e.ctrlKey && !e.metaKey;
        const action =
          ctrl && e.code === "KeyK" ? "toggle-keyboard" :
          ctrl && e.code === "KeyC" ? "copy" :
          ctrl && e.code === "KeyV" ? "paste" :
          ctrl && e.code === "KeyX" ? "cut" :
          ctrl && e.code === "KeyA" ? "select-all" :
          ctrl && e.code === "KeyZ" ? "undo" :
          alt && e.code === "KeyV" ? "paste-pc" :
          alt && e.code === "KeyL" ? "lock" :
          alt && e.code === "KeyU" ? "wake" : null;
        if (alt && ["KeyH", "KeyC", "KeyR", "KeyD"].includes(e.code)) {  // app shortcuts (app.js)
          ignoreKeyUpUntil = Date.now() + 400;
          return;
        }
        if (action) {
          e.preventDefault();
          e.stopPropagation();
          ignoreKeyUpUntil = Date.now() + 400;
          pressed.clear();
          flush();
          runAction(action);
          return;
        }
        // Ctrl alone must not reach iOS as a modifier (Ctrl+letter is mapped above).
        if (e.code === "ControlLeft" || e.code === "ControlRight") return;

        // Regular keystroke
        const usage = CODE_TO_HID[e.code];
        if (usage === undefined) return;
        e.preventDefault();
        e.stopPropagation();
        if (e.repeat) return;
        pressed.add(usage);
        flush();
      },
      true,
    );

    window.addEventListener(
      "keyup",
      (e) => {
        if (Date.now() < ignoreKeyUpUntil) {
          e.preventDefault();
          return;
        }
        if (e.code === "ControlLeft" || e.code === "ControlRight") return;
        const usage = CODE_TO_HID[e.code];
        if (usage !== undefined && pressed.delete(usage)) {
          e.preventDefault();
          flush();
        }
      },
      true,
    );

    window.addEventListener("blur", releaseAll);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState !== "visible") releaseAll();
    });
    return { releaseAll };
  }

  window.HandsetKeys = { attach, runAction };
})();
