//! Real Windows translucency behind the window (not a painted imitation).
//!
//! Windows 11 22H2+: DWM system backdrop (Acrylic "transient window").
//! DWM drops Acrylic to a solid colour while the window is inactive, so the
//! fallback `accent` mode uses SetWindowCompositionAttribute, which keeps the
//! blur-behind on whether or not the window has focus.

use windows::core::BOOL;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMSBT_NONE, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWM_SYSTEMBACKDROP_TYPE,
};
use windows::Win32::UI::Controls::MARGINS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    /// No translucency: solid background (glass intensity 0%).
    None,
    /// Blur-behind that stays on while inactive.
    Accent,
}

#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    gradient: u32, // AABBGGRR tint
    animation: u32,
}

#[repr(C)]
struct CompositionData {
    attribute: u32,
    data: *mut core::ffi::c_void,
    size: usize,
}

type SetWca = unsafe extern "system" fn(HWND, *mut CompositionData) -> BOOL;

fn set_accent(hwnd: HWND, state: u32, tint: u32) -> bool {
    use windows::core::s;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    unsafe {
        let Ok(user32) = GetModuleHandleA(s!("user32.dll")) else {
            return false;
        };
        let Some(proc_) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else {
            return false;
        };
        let f: SetWca = core::mem::transmute(proc_);
        let mut policy = AccentPolicy {
            state,
            flags: 2,
            gradient: tint,
            animation: 0,
        };
        let mut data = CompositionData {
            attribute: 19, // WCA_ACCENT_POLICY
            data: &mut policy as *mut _ as *mut _,
            size: core::mem::size_of::<AccentPolicy>(),
        };
        f(hwnd, &mut data).as_bool()
    }
}

/// Apply `mode` to `hwnd`. `dark` picks the dark title bar and tint.
/// `thick` (0..1): 0 = fully clear (no blur at all, the desktop shows as
/// is), higher = Windows Acrylic blur with a stronger tint.
/// `round`: Windows 11 rounded corners (off in full screen / maximised).
/// Dark or light system title bar (app windows keep the Windows frame).
pub fn dark_title_bar(hwnd: HWND, dark: bool) {
    let d = BOOL::from(dark);
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &d as *const _ as _, 4);
    }
}

pub fn apply(hwnd: HWND, mode: Backdrop, dark: bool, thick: f32, round: bool) -> bool {
    unsafe {
        let d = BOOL::from(dark);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &d as *const _ as _, 4);
        // Rounded corners like Windows 11 / macOS windows (no system frame).
        let corner: u32 = if round { 2 } else { 1 }; // DWMWCP_ROUND / DWMWCP_DONOTROUND
        let _ = DwmSetWindowAttribute(
            hwnd,
            windows::Win32::Graphics::Dwm::DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as _,
            4,
        );
        // No frame extension: on a frameless window DWM would paint ghost
        // caption buttons into the extended area. The blur needs none.
        let margins = MARGINS::default();
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        let set_backdrop = |t: DWM_SYSTEMBACKDROP_TYPE| {
            DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &t as *const _ as _, 4).is_ok()
        };
        match mode {
            Backdrop::None => {
                set_accent(hwnd, 0, 0);
                set_backdrop(DWMSBT_NONE)
            }
            Backdrop::Accent if thick < 0.03 => {
                // Clear: no blur, per-pixel alpha only (winit transparency).
                set_backdrop(DWMSBT_NONE);
                set_accent(hwnd, 0, 0)
            }
            Backdrop::Accent => {
                set_backdrop(DWMSBT_NONE);
                let glass = 1.0 - thick;
                // ACCENT_ENABLE_ACRYLICBLURBEHIND = 4, light/dark tint with low alpha.
                // Tint alpha follows the glass level: ~0 at 100% gives clear
                // glass (measured: background stripes stay visible), up to
                // a frosted 0xB0 near 0%. Colour is AABBGGRR.
                let a = (((1.0 - glass.clamp(0.0, 1.0)) * 0xB0 as f32) as u32).max(1);
                // The tint colour also drives Acrylic's luminosity layer: a
                // white tint turns the glass milky even at alpha 1. Neutral
                // grey keeps it clear (measured 6x more background through);
                // the theme colour comes in as the glass level drops.
                let g = glass.clamp(0.0, 1.0);
                let mix = |c: u32, n: u32| -> u32 {
                    let ch = |sh: u32| {
                        let (a, b) = (((c >> sh) & 0xff) as f32, ((n >> sh) & 0xff) as f32);
                        ((a + (b - a) * g) as u32) << sh
                    };
                    ch(0) | ch(8) | ch(16)
                };
                // Neutral grey per theme: mid-dark for dark glass (no washed-out grey),
                // light grey for light glass.
                let rgb = mix(
                    if dark { 0x1e_1c_1c } else { 0xf7_f2_f2 },
                    if dark { 0x2c_2c_2c } else { 0xa8_a8_a8 },
                ); // AABBGGRR
                set_accent(hwnd, 4, (a << 24) | rgb)
            }
        }
    }
}
