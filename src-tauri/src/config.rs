//! Persistent per-widget configuration.
//!
//! Each widget window owns one row in a tiny JSON file living next to
//! the application's other config under the OS-standard config dir
//! (`~/.config/dev.sysdashboard.app/widgets.json` on Linux). The row
//! remembers two things:
//!
//!   * Physical screen position of the window, so on the next launch
//!     the widget pops up exactly where the user left it.
//!   * Whether the user clicked the lock icon - locked widgets cannot
//!     be dragged (the frontend strips `data-tauri-drag-region`).
//!
//! Concurrency: the in-memory copy is wrapped in a `Mutex` because the
//! `WindowEvent::Moved` callback fires from a Tauri-owned thread and
//! the `set_widget_locked` command can fire concurrently from another.
//! Writes hit disk synchronously - the file is < 1 KiB, so even at the
//! event rate of an active drag (tens of events per second) the I/O is
//! negligible.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// One row per widget. `x`/`y` are physical pixels (what `WindowEvent::Moved`
/// hands us) and what we feed back to `set_position` on restore - this
/// avoids any HiDPI scale-factor drift between sessions. `width`/`height`
/// are physical pixels too, sourced from `WindowEvent::Resized`.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct WidgetConfig {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(default)]
    pub locked: bool,
}

/// Top-level file shape:
/// `{ "place": "Best City Ever", "widgets": { "cpu": {...}, ... } }`.
///
/// `place` is the town/city name passed to the FMI Open Data WFS
/// (`place=` query arg) by [`crate::weather::fmi::FmiProvider`]. Missing
/// or empty values fall back to `"Helsinki"` - see [`ConfigStore::place`].
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub place: Option<String>,
    #[serde(default)]
    pub widgets: HashMap<String, WidgetConfig>,
}

const DEFAULT_PLACE: &str = "Helsinki";

/// Loaded-and-mutable view of `widgets.json`.
///
/// Store this in `tauri::Manager` state wrapped in an `Arc` so the move
/// callback closures registered per window can share ownership with
/// the command handlers.
pub struct ConfigStore {
    path: PathBuf,
    inner: Mutex<Config>,
}

impl ConfigStore {
    /// Read `path`, or fall back to an empty config if the file is
    /// missing, unreadable, or corrupt. A corrupt file is silently
    /// replaced the first time we write - we explicitly do not want a
    /// transient parse error to lose the user's positions, but a truly
    /// unreadable file means "first launch on this machine" and we
    /// proceed with defaults.
    pub fn load(path: PathBuf) -> Self {
        let inner = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
            .unwrap_or_default();
        Self {
            path,
            inner: Mutex::new(inner),
        }
    }

    /// Currently-configured town/city name passed to the weather
    /// provider. Falls back to `"Helsinki"` on a missing, blank, or
    /// whitespace-only value so the forecast widget always has
    /// *something* to query.
    pub fn place(&self) -> String {
        self.inner
            .lock()
            .unwrap()
            .place
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| DEFAULT_PLACE.to_owned())
    }

    /// Snapshot of one widget's config. Returns defaults for unknown
    /// labels so callers can treat first-launch and unknown-widget
    /// the same way.
    pub fn get(&self, label: &str) -> WidgetConfig {
        self.inner
            .lock()
            .unwrap()
            .widgets
            .get(label)
            .cloned()
            .unwrap_or_default()
    }

    /// Record the latest physical position and flush.
    pub fn set_position(&self, label: &str, x: i32, y: i32) {
        let mut cfg = self.inner.lock().unwrap();
        let entry = cfg.widgets.entry(label.to_string()).or_default();
        entry.x = Some(x);
        entry.y = Some(y);
        let _ = self.write_locked(&cfg);
    }

    /// Record the latest physical size and flush.
    pub fn set_size(&self, label: &str, width: u32, height: u32) {
        let mut cfg = self.inner.lock().unwrap();
        let entry = cfg.widgets.entry(label.to_string()).or_default();
        entry.width = Some(width);
        entry.height = Some(height);
        let _ = self.write_locked(&cfg);
    }

    /// Record the lock state and flush.
    pub fn set_locked(&self, label: &str, locked: bool) {
        let mut cfg = self.inner.lock().unwrap();
        let entry = cfg.widgets.entry(label.to_string()).or_default();
        entry.locked = locked;
        let _ = self.write_locked(&cfg);
    }

    fn write_locked(&self, cfg: &Config) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(cfg).unwrap();
        fs::write(&self.path, json)
    }
}
