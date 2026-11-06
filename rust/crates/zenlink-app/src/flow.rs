//! Startup navigation (master prompt section 2, "Mandatory startup flow").
//!
//! Launch -> platform selection -> iPhone/iPad (the v2.2.31 Apple Driver /
//! ZenLink Studio chooser, unchanged) or Android Phone/Tablet.
//!
//! Kept free of Slint types so the acceptance rules are unit-tested here; the
//! window only mirrors `Flow` into its properties.

/// Which screen the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// The new first screen: "iPhone / iPad" or "Android Phone / Tablet".
    Platform,
    /// The Apple workspace (with its own mode chooser as step two).
    Apple,
    /// The Android workspace.
    Android,
}

impl Screen {
    pub fn as_str(self) -> &'static str {
        match self {
            Screen::Platform => "platform",
            Screen::Apple => "apple",
            Screen::Android => "android",
        }
    }
}

/// A saved platform choice ("" = none yet).
pub fn parse_platform(s: &str) -> Option<Screen> {
    match s {
        "apple" => Some(Screen::Apple),
        "android" => Some(Screen::Android),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flow {
    pub screen: Screen,
    /// The Apple mode chooser (Apple Driver / ZenLink Studio) is open.
    pub apple_chooser: bool,
    /// The card highlighted on the platform screen (kept across Back).
    pub picked: Option<Screen>,
}

impl Flow {
    /// At launch. `platform` and `remember` come from the window prefs;
    /// `apple_mode_remembered` is the backend's own "Remember my choice"
    /// (separate setting, unchanged from v2.2.31).
    pub fn launch(platform: &str, remember: bool, apple_mode_remembered: bool) -> Self {
        let saved = parse_platform(platform);
        let screen = match saved {
            Some(s) if remember => s,
            _ => Screen::Platform,
        };
        Flow {
            screen,
            apple_chooser: screen == Screen::Apple && !apple_mode_remembered,
            picked: saved,
        }
    }

    /// The user pressed Continue on the platform screen.
    pub fn choose(&mut self, platform: Screen, apple_needs_choice: bool) {
        self.picked = Some(platform);
        self.screen = platform;
        // iPhone/iPad goes to the existing chooser, never straight to
        // mirroring or a driver change; Android never opens it.
        self.apple_chooser = platform == Screen::Apple && apple_needs_choice;
    }

    /// "Back / Change device type" from either workspace or the Apple chooser.
    pub fn back(&mut self) {
        self.screen = Screen::Platform;
        self.apple_chooser = false;
    }

    /// The backend reports that no Apple mode is chosen yet. Only the Apple
    /// workspace may show the chooser for it.
    pub fn backend_needs_choice(&mut self) {
        if self.screen == Screen::Apple {
            self.apple_chooser = true;
        }
    }

    /// Apple capture may run only in the Apple workspace.
    pub fn apple_active(&self) -> bool {
        self.screen == Screen::Apple
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_install_shows_platform_before_apple_chooser() {
        let f = Flow::launch("", false, false);
        assert_eq!(f.screen, Screen::Platform);
        assert!(!f.apple_chooser);
        assert_eq!(f.picked, None);
    }

    #[test]
    fn old_config_without_platform_sees_selector_even_with_apple_mode_remembered() {
        // v2.2.31 / earlier beta prefs: no platform key, Apple mode remembered.
        let f = Flow::launch("", false, true);
        assert_eq!(f.screen, Screen::Platform);
        assert!(!f.apple_chooser);
    }

    #[test]
    fn iphone_path_opens_existing_chooser() {
        let mut f = Flow::launch("", false, false);
        f.choose(Screen::Apple, true);
        assert_eq!(f.screen, Screen::Apple);
        assert!(f.apple_chooser);
        // A remembered Apple mode skips the chooser, as in v2.2.31.
        let mut g = Flow::launch("", false, true);
        g.choose(Screen::Apple, false);
        assert!(!g.apple_chooser);
    }

    #[test]
    fn android_path_never_opens_apple_chooser() {
        let mut f = Flow::launch("", false, false);
        f.choose(Screen::Android, true);
        assert_eq!(f.screen, Screen::Android);
        assert!(!f.apple_chooser);
        f.backend_needs_choice();
        assert!(
            !f.apple_chooser,
            "backend state must not pop the Apple chooser over Android"
        );
        assert!(!f.apple_active());
    }

    #[test]
    fn back_from_both_paths_keeps_selection() {
        for p in [Screen::Apple, Screen::Android] {
            let mut f = Flow::launch("", false, false);
            f.choose(p, true);
            f.back();
            assert_eq!(f.screen, Screen::Platform);
            assert!(!f.apple_chooser);
            assert_eq!(f.picked, Some(p), "selection preserved after Back");
            f.choose(p, false);
            assert_eq!(f.screen, p);
        }
    }

    #[test]
    fn remembered_platform_is_followed_only_when_opted_in() {
        assert_eq!(Flow::launch("android", true, false).screen, Screen::Android);
        assert_eq!(
            Flow::launch("android", false, false).screen,
            Screen::Platform
        );
        assert_eq!(
            Flow::launch("android", false, false).picked,
            Some(Screen::Android)
        );
        let a = Flow::launch("apple", true, false);
        assert_eq!(a.screen, Screen::Apple);
        assert!(
            a.apple_chooser,
            "Apple chooser still asks when its own choice is not remembered"
        );
        assert!(!Flow::launch("apple", true, true).apple_chooser);
        assert_eq!(Flow::launch("bogus", true, true).screen, Screen::Platform);
    }

    #[test]
    fn backend_needs_choice_only_in_apple() {
        let mut f = Flow::launch("", false, false);
        f.backend_needs_choice();
        assert!(!f.apple_chooser, "not over the platform screen");
        f.choose(Screen::Apple, false);
        f.backend_needs_choice();
        assert!(f.apple_chooser);
    }
}
