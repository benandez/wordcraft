//! Preferences between visits (#2132): the same [`UiState`] the desktop app keeps in `ui.json`
//! (theme, interface size, recent symbols, editing options…), kept in the browser's
//! `localStorage` under [`KEY`]. Restored at startup; written back whenever it changes, so a
//! reload or a closed tab keeps the last setting (browsers don't reliably run exit code).

use wordcraft_ui_egui::{UiState, WordApp};

/// The `localStorage` key.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub const KEY: &str = "wordcraft.ui";
/// Seconds between checks for changed preferences: building and serializing them (the user
/// dictionary, keymap, ribbon layout…) every frame is wasted work while typing or dragging.
pub const CHECK_EVERY: f64 = 0.5;
/// Largest stored value read back (`localStorage` holds about 5 MB per site).
const MAX_STORED: usize = 4 << 20;

/// Apply preferences stored on an earlier visit. Missing, oversized or unreadable values leave
/// the defaults.
pub fn restore(app: &mut WordApp, stored: Option<&str>) {
    if let Some(s) = stored.filter(|s| s.len() <= MAX_STORED)
        && let Some(ui) = UiState::from_json(s.as_bytes())
    {
        app.apply_prefs(ui);
    }
}

/// Tracks what was last written, so the store is written only when a preference changed.
#[derive(Default)]
pub struct PrefsSync {
    last: Option<String>,
    /// When preferences were last compared (`egui` input time, seconds).
    checked: Option<f64>,
    /// A check was skipped since the last one, so another is owed.
    owed: bool,
}

impl PrefsSync {
    /// Starts from what is already stored, so restoring doesn't write it straight back.
    pub fn new(stored: Option<String>) -> Self {
        Self { last: stored, checked: None, owed: false }
    }

    /// Whether a check was skipped and is still owed: the caller repaints after [`CHECK_EVERY`]
    /// so a change made just before the app goes idle is still stored.
    pub fn owed(&self) -> bool {
        self.owed
    }

    /// The JSON to store when the app's preferences differ from what was last written. Compares
    /// at most every [`CHECK_EVERY`] seconds of `now`.
    pub fn changed(&mut self, app: &WordApp, now: f64) -> Option<String> {
        if self.checked.is_some_and(|t| (0.0..CHECK_EVERY).contains(&(now - t))) {
            self.owed = true;
            return None;
        }
        self.checked = Some(now);
        self.owed = false;
        let json = match serde_json::to_string(&app.prefs()) {
            Ok(j) => j,
            Err(e) => {
                log::warn!("preferences: cannot serialize: {e}");
                return None;
            }
        };
        if self.last.as_deref() == Some(json.as_str()) {
            return None;
        }
        self.last = Some(json.clone());
        Some(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wordcraft_ui_egui::Services;

    fn app() -> WordApp {
        WordApp::new(wordcraft_engine::Session::new(wordcraft_engine::sample::sample_document()), Services::default())
    }

    /// #2132: dark mode chosen on one visit is there after a reload.
    #[test]
    fn dark_mode_survives_reload() {
        let mut first = app();
        let mut sync = PrefsSync::new(None);
        let initial = sync.changed(&first, 0.0);
        assert!(initial.is_some(), "first frame stores the preferences");
        assert_eq!(sync.changed(&first, 1.0), None, "unchanged preferences are not rewritten");
        first.run("ui.theme", json!({"value": "dark"})).unwrap();
        assert_eq!(sync.changed(&first, 1.2), None, "not compared again within CHECK_EVERY");
        assert!(sync.owed(), "the skipped check is owed");
        let stored = sync.changed(&first, 1.6).expect("the theme change is stored");
        assert!(!sync.owed());

        let mut reloaded = app();
        restore(&mut reloaded, Some(&stored));
        assert_eq!(reloaded.ui.theme, wordcraft_ui_egui::theme::Appearance::Dark);
        assert_eq!(PrefsSync::new(Some(stored)).changed(&reloaded, 0.0), None, "restoring doesn't write back");
    }

    #[test]
    fn junk_storage_keeps_defaults() {
        for junk in [None, Some(""), Some("not json"), Some("[1,2]"), Some("{\"theme\":42}")] {
            let mut a = app();
            restore(&mut a, junk);
            assert_eq!(a.ui.theme, wordcraft_ui_egui::theme::Appearance::Light);
        }
    }
}
