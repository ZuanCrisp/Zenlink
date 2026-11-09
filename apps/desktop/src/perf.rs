//! `ZL_PERF=1`: logs every 5 s how long the window's rendering and the frame timer take,
//! so UI cost is measured, not guessed.

use std::cell::Cell;
use std::time::{Duration, Instant};

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static RENDER: Cell<(Duration, u32)> = const { Cell::new((Duration::ZERO, 0)) };
    static TICK: Cell<(Duration, u32)> = const { Cell::new((Duration::ZERO, 0)) };
    static SINCE: Cell<Option<Instant>> = const { Cell::new(None) };
}

fn add(c: &'static std::thread::LocalKey<Cell<(Duration, u32)>>, d: Duration) {
    c.with(|c| {
        let (t, n) = c.get();
        c.set((t + d, n + 1));
    });
}

pub fn install(ui: &crate::AppWindow) {
    use slint::ComponentHandle;
    ON.with(|o| o.set(true));
    let start = Cell::new(None::<Instant>);
    let _ = ui.window().set_rendering_notifier(move |state, _| match state {
        slint::RenderingState::BeforeRendering => start.set(Some(Instant::now())),
        slint::RenderingState::AfterRendering => {
            if let Some(t) = start.take() {
                add(&RENDER, t.elapsed());
            }
        }
        _ => {}
    });
}

/// Times one frame-timer tick (on drop).
pub struct Tick(Option<Instant>);

impl Tick {
    pub fn new() -> Self {
        Self(ON.with(Cell::get).then(Instant::now))
    }
}

impl Drop for Tick {
    fn drop(&mut self) {
        let Some(t) = self.0 else { return };
        add(&TICK, t.elapsed());
        let since = SINCE.with(|s| *s.get().get_or_insert_with(|| {
            s.set(Some(Instant::now()));
            Instant::now()
        }));
        if since.elapsed() < Duration::from_secs(5) {
            return;
        }
        SINCE.with(|s| s.set(Some(Instant::now())));
        let (rt, rn) = RENDER.with(|c| c.replace((Duration::ZERO, 0)));
        let (tt, tn) = TICK.with(|c| c.replace((Duration::ZERO, 0)));
        let ms = |t: Duration, n: u32| t.as_secs_f64() * 1000.0 / f64::from(n.max(1));
        log::info!(
            "perf: render {rn} frames, {:.2} ms each; timer {tn} ticks, {:.2} ms each",
            ms(rt, rn),
            ms(tt, tn)
        );
    }
}
