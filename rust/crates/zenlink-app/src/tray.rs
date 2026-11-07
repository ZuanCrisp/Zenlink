//! Notification-area icon ("Close to the tray"), "Start with Windows" and
//! one window per user (B-29).
//!
//! A hidden helper window (class `ZenLinkTrayV25`) owns the tray icon. It is
//! created on the UI thread, so winit's message loop dispatches its messages
//! too; no extra thread. A second `ZenLink.exe` finds that window, asks it to
//! show the running ZenLink, and exits.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, FindWindowW,
    GetCursorPos, GetSystemMetrics, LoadIconW, LoadImageW, PostMessageW, RegisterClassW,
    RegisterWindowMessageW, SetForegroundWindow, TrackPopupMenu, HICON, IDI_APPLICATION,
    IMAGE_ICON, LR_DEFAULTCOLOR, MF_SEPARATOR, MF_STRING, SM_CXSMICON, SM_CYSMICON,
    TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_APP, WM_CONTEXTMENU,
    WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};

const CLASS: PCWSTR = w!("ZenLinkTrayV251Beta");
const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = w!("ZenLink Beta");
/// Command-line flag of the "Start with Windows" entry.
pub const BACKGROUND_ARG: &str = "--background";
const WM_TRAY: u32 = WM_APP + 0x51;
const WM_SHOW_OTHER: u32 = WM_APP + 0x52;
const ID_OPEN: usize = 1;
const ID_QUIT: usize = 2;

/// What the tray asks the window to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Left click: show the window, or hide it when it is shown.
    Toggle,
    /// Menu "Open", or a second ZenLink.exe was started.
    Show,
    Quit,
}

type Handler = Rc<dyn Fn(Event)>;

/// True while the window is hidden in the tray (no work for it then).
pub static HIDDEN: AtomicBool = AtomicBool::new(false);

thread_local! {
    static HWND_TRAY: Cell<Option<isize>> = const { Cell::new(None) };
    static HANDLER: RefCell<Option<Handler>> = const { RefCell::new(None) };
    static LABELS: RefCell<(String, String)> = RefCell::new(("Open ZenLink".into(), "Quit".into()));
    static ICON_ON: Cell<bool> = const { Cell::new(false) };
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// When another ZenLink v2.5 runs for this user: ask it to show its window
/// and return true (the caller exits).
pub fn show_running_instance() -> bool {
    match unsafe { FindWindowW(CLASS, PCWSTR::null()) } {
        Ok(h) if !h.is_invalid() => {
            let _ = unsafe { PostMessageW(Some(h), WM_SHOW_OTHER, WPARAM(0), LPARAM(0)) };
            true
        }
        _ => false,
    }
}

/// Creates the helper window. `handler` runs on the UI thread; it should
/// defer Slint work with `slint::invoke_from_event_loop`.
pub fn install(handler: impl Fn(Event) + 'static) {
    HANDLER.with_borrow_mut(|h| *h = Some(Rc::new(handler)));
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: module.into(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassW(&wc);
        TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
        // A normal (never shown) top-level window: message-only windows do not
        // get "TaskbarCreated" after Explorer restarts.
        if let Ok(h) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            w!("ZenLink"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(module.into()),
            None,
        ) {
            HWND_TRAY.set(Some(h.0 as isize));
        }
    }
}

/// Shows or removes the tray icon and sets the menu language.
pub fn set_icon(on: bool, open: &str, quit: &str) {
    LABELS.with_borrow_mut(|l| *l = (open.to_string(), quit.to_string()));
    if on == ICON_ON.get() {
        return;
    }
    if notify(if on { NIM_ADD } else { NIM_DELETE }) || !on {
        ICON_ON.set(on);
    }
}

fn notify(op: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE) -> bool {
    let Some(h) = HWND_TRAY.get() else {
        return false;
    };
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: HWND(h as _),
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: app_icon(),
        ..Default::default()
    };
    for (d, s) in nid.szTip.iter_mut().zip("ZenLink Beta".encode_utf16()) {
        *d = s;
    }
    unsafe { Shell_NotifyIconW(op, &nid).as_bool() }
}

/// The exe's icon (resource 1, see build.rs) at the small-icon size.
fn app_icon() -> HICON {
    unsafe {
        let module = GetModuleHandleW(None).ok();
        let img = LoadImageW(
            module.map(Into::into),
            PCWSTR(1 as _),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        );
        match img {
            Ok(h) if !h.is_invalid() => HICON(h.0),
            _ => LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
        }
    }
}

fn emit(e: Event) {
    // Clone out first: the handler may run while a menu loop is open.
    let h = HANDLER.with_borrow(|h| h.clone());
    if let Some(h) = h {
        h(e);
    }
}

fn menu(hwnd: HWND) -> Option<Event> {
    let (open, quit) = LABELS.with_borrow(|l| (wide(&l.0), wide(&l.1)));
    unsafe {
        let m = CreatePopupMenu().ok()?;
        let _ = AppendMenuW(m, MF_STRING, ID_OPEN, PCWSTR(open.as_ptr()));
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(m, MF_STRING, ID_QUIT, PCWSTR(quit.as_ptr()));
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Without this the menu does not close when the user clicks elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            m,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(m);
        match cmd.0 as usize {
            ID_OPEN => Some(Event::Show),
            ID_QUIT => Some(Event::Quit),
            _ => None,
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            match lp.0 as u32 {
                WM_LBUTTONUP => emit(Event::Toggle),
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    if let Some(e) = menu(hwnd) {
                        emit(e);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_SHOW_OTHER => {
            emit(Event::Show);
            LRESULT(0)
        }
        m if m != 0 && m == TASKBAR_CREATED.get() => {
            // Explorer restarted: the icon is gone, add it again.
            if ICON_ON.get() {
                notify(NIM_ADD);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

/// The "Start with Windows" command line for this exe.
pub fn run_command(exe: &std::path::Path) -> String {
    format!("\"{}\" {BACKGROUND_ARG}", exe.display())
}

/// The current HKCU Run value, if any.
fn run_value() -> Option<String> {
    let mut buf = vec![0u16; 1024];
    let mut len = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            RUN_VALUE,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
    };
    if r.is_err() {
        return None;
    }
    let n = (len as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]))
}

/// Writes or removes the HKCU Run entry. Only the user's own key: no admin
/// rights, no scheduled task, nothing hidden (AGENTS.md §3c). Removing at
/// start-up ("off") only touches an entry that points to this exe, so a
/// second copy that the user set to start is left alone.
pub fn set_start_with_windows(on: bool, user_action: bool) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let want = run_command(&exe);
    let have = run_value();
    if on {
        if have.as_deref() != Some(want.as_str()) {
            let data = wide(&want);
            let r = unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    RUN_KEY,
                    RUN_VALUE,
                    REG_SZ.0,
                    Some(data.as_ptr().cast()),
                    (data.len() * 2) as u32,
                )
            };
            if r.is_err() {
                log::warn!("start with Windows: could not write the Run entry: {r:?}");
            }
        }
    } else if have.is_some() && (user_action || have.as_deref() == Some(want.as_str())) {
        let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE) };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn run_command_quotes_the_path_and_starts_in_the_background() {
        let p = std::path::Path::new(r"C:\Program Files\ZenLink\ZenLink.exe");
        assert_eq!(
            super::run_command(p),
            r#""C:\Program Files\ZenLink\ZenLink.exe" --background"#
        );
    }
}
