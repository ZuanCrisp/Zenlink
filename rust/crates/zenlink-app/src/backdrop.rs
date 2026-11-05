//! Real Windows translucency behind the window (not a painted imitation).
//!
//! Windows 11 22H2+: DWM system backdrop (Acrylic "transient window").
//! DWM drops Acrylic to a solid colour while the window is inactive, so the
//! fallback `accent` mode uses SetWindowCompositionAttribute, which keeps the
//! blur-behind on whether or not the window has focus.

use windows::core::BOOL;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW,
    DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DWM_SYSTEMBACKDROP_TYPE,
};
use windows::Win32::UI::Controls::MARGINS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    /// No translucency: solid background (glass intensity 0%).
    None,
    /// DWM Acrylic (system backdrop). Solid while the window is inactive.
    Acrylic,
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
        let Ok(user32) = GetModuleHandleA(s!("user32.dll")) else { return false };
        let Some(proc_) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else { return false };
        let f: SetWca = core::mem::transmute(proc_);
        let mut policy = AccentPolicy { state, flags: 2, gradient: tint, animation: 0 };
        let mut data = CompositionData {
            attribute: 19, // WCA_ACCENT_POLICY
            data: &mut policy as *mut _ as *mut _,
            size: core::mem::size_of::<AccentPolicy>(),
        };
        f(hwnd, &mut data).as_bool()
    }
}

/// Apply `mode` to `hwnd`. `dark` picks the dark title bar and tint.
pub fn apply(hwnd: HWND, mode: Backdrop, dark: bool) -> bool {
    unsafe {
        let d = BOOL::from(dark);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &d as *const _ as _, 4);
        let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        let set_backdrop = |t: DWM_SYSTEMBACKDROP_TYPE| {
            DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &t as *const _ as _, 4).is_ok()
        };
        match mode {
            Backdrop::None => {
                set_accent(hwnd, 0, 0);
                set_backdrop(DWMSBT_NONE)
            }
            Backdrop::Acrylic => {
                set_accent(hwnd, 0, 0);
                set_backdrop(DWMSBT_TRANSIENTWINDOW)
            }
            Backdrop::Accent => {
                set_backdrop(DWMSBT_NONE);
                // ACCENT_ENABLE_ACRYLICBLURBEHIND = 4, light/dark tint with low alpha.
                let tint = if dark { 0x30_16_11_10 } else { 0x28_fa_f6_f5 };
                set_accent(hwnd, 4, tint)
            }
        }
    }
}
