//! Application library: builds the Tauri app, registers the long-lived
//! state, and exposes the `#[tauri::command]` functions the frontend
//! invokes.
//!
//! Architecture:
//!
//! ```text
//!     JS invoke()          tauri::Builder
//!          |                     |
//!          v                     v
//!   #[tauri::command]  ---->  AppState        (Mutex<SystemMonitor>,
//!                                              Mutex<GpuMonitor>,
//!                                              Box<dyn WeatherProvider>)
//!                          ConfigStore        (Mutex<Config>, file path)
//!                          + N WebviewWindows (cpu, ram, gpu, forecast)
//! ```
//!
//! Multi-window layout: every widget is its own top-level window so the
//! user can grab, move and lock each one independently. Windows are
//! built programmatically inside `setup` (rather than declared in
//! `tauri.conf.json`) because we want their position to come from a
//! user-writable JSON file on every launch.

mod config;
mod monitors;
mod weather;

use std::sync::{Arc, Mutex};

use tauri::{
    Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};

use config::ConfigStore;
use monitors::cpu_ram::SystemMonitor;
use monitors::gpu::GpuMonitor;
use weather::demo::DemoProvider;
use weather::{Forecast, WeatherProvider};

/// One entry per widget window we want to spawn. `default_x`/`default_y`
/// are used on the very first launch (or when the user deletes the
/// config file). After that, `ConfigStore` supplies the real position.
struct WidgetSpec {
    label: &'static str,
    width: f64,
    height: f64,
    default_x: f64,
    default_y: f64,
}

const WIDGETS: &[WidgetSpec] = &[
    WidgetSpec { label: "cpu",      width: 120.0, height: 140.0, default_x: 60.0,  default_y: 60.0  },
    WidgetSpec { label: "ram",      width: 120.0, height: 140.0, default_x: 200.0, default_y: 60.0  },
    WidgetSpec { label: "gpu",      width: 120.0, height: 140.0, default_x: 340.0, default_y: 60.0  },
    WidgetSpec { label: "forecast", width: 260.0, height: 260.0, default_x: 60.0,  default_y: 220.0 },
];

/// Long-lived data the Tauri app owns. See module docstring for the
/// reasoning behind each `Mutex` / `Box`.
pub struct AppState {
    pub system: Mutex<SystemMonitor>,
    pub gpu: Mutex<GpuMonitor>,
    pub weather: Box<dyn WeatherProvider>,
}

/// JSON payload emitted each tick.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStats {
    pub cpu_percent: f32,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub gpu_percent: Option<f32>,
    pub vram_used_bytes: Option<u64>,
    pub vram_total_bytes: Option<u64>,
}

#[tauri::command]
fn get_system_stats(state: tauri::State<AppState>) -> Result<SystemStats, String> {
    let mut sys = state.system.lock().map_err(|e| e.to_string())?;
    let mut gpu = state.gpu.lock().map_err(|e| e.to_string())?;

    let (cpu_percent, ram_used_bytes, ram_total_bytes) = sys.sample();
    let gpu_stats = gpu.sample();

    Ok(SystemStats {
        cpu_percent,
        ram_used_bytes,
        ram_total_bytes,
        gpu_percent: gpu_stats.as_ref().map(|s| s.utilization_percent),
        vram_used_bytes: gpu_stats.as_ref().map(|s| s.vram_used_bytes),
        vram_total_bytes: gpu_stats.as_ref().map(|s| s.vram_total_bytes),
    })
}

#[tauri::command]
fn get_forecast(state: tauri::State<AppState>) -> Result<Forecast, String> {
    state.weather.forecast()
}

/// Returns whether the named widget is currently locked. The frontend
/// calls this on load to decide whether to render the open or closed
/// lock icon and whether to keep `data-tauri-drag-region` on the
/// header.
#[tauri::command]
fn get_widget_locked(label: String, store: tauri::State<Arc<ConfigStore>>) -> bool {
    store.get(&label).locked
}

/// Persist a new lock state. The frontend is responsible for the
/// purely-cosmetic side (icon swap + drag-region attribute toggle);
/// this command just records the user's intent so it survives
/// restarts.
#[tauri::command]
fn set_widget_locked(label: String, locked: bool, store: tauri::State<Arc<ConfigStore>>) {
    store.set_locked(&label, locked);
}

/// Application entry point - called from `main.rs`.
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            system: Mutex::new(SystemMonitor::new()),
            gpu: Mutex::new(GpuMonitor::new()),
            // TODO(you): swap DemoProvider for your real implementation.
            weather: Box::new(DemoProvider::default()),
        })
        .setup(|app| {
            // Resolve the config file location. `app_config_dir()` returns
            // `~/.config/<identifier>/` on Linux, with platform-equivalent
            // paths elsewhere. Failing here is fatal: we can't run without
            // somewhere to remember positions.
            let cfg_path = app
                .path()
                .app_config_dir()
                .expect("no app config dir")
                .join("widgets.json");

            let store = Arc::new(ConfigStore::load(cfg_path));
            // Make the store retrievable by commands as State<Arc<ConfigStore>>.
            app.manage(store.clone());

            for spec in WIDGETS {
                let cfg = store.get(spec.label);
                let url = WebviewUrl::App(
                    format!("index.html?widget={}", spec.label).into(),
                );

                // Build the window. We hand it the *default* logical
                // position and size; if we have saved physical values
                // we override them below.
                //
                // resizable(true) lets the frontend's corner grip call
                // `startResizeDragging`. On a frameless window the OS
                // doesn't draw edge handles, so the grip is the only
                // way the user can change the size.
                let win = WebviewWindowBuilder::new(app, spec.label, url)
                    .title(spec.label)
                    .inner_size(spec.width, spec.height)
                    .min_inner_size(72.0, 72.0)
                    .position(spec.default_x, spec.default_y)
                    .resizable(true)
                    .decorations(false)
                    .transparent(true)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .shadow(false)
                    .build()?;

                if let (Some(x), Some(y)) = (cfg.x, cfg.y) {
                    // Skip the (0, 0) sentinel left over from any
                    // earlier native-Wayland session. See main.rs for
                    // why xdg_toplevel reports zeros instead of real
                    // coordinates; the leftover values would otherwise
                    // stack every widget in the top-left corner on the
                    // first launch after the X11-backend fix. A user
                    // who genuinely *wants* a widget at (0, 0) only
                    // has to nudge it one pixel for a real value to
                    // be persisted.
                    if x != 0 || y != 0 {
                        // Saved positions are physical pixels (what
                        // WindowEvent::Moved gives us), so restore as
                        // physical to avoid HiDPI scale drift.
                        let _ = win.set_position(Position::Physical(
                            PhysicalPosition::new(x, y),
                        ));
                    }
                }
                if let (Some(w), Some(h)) = (cfg.width, cfg.height) {
                    let _ = win.set_size(Size::Physical(PhysicalSize::new(w, h)));
                }

                // Persist every move and resize. The callback runs on
                // a Tauri event-loop thread, so the Arc<ConfigStore>
                // clone we hand it must be Send + Sync (which it is).
                let label_owned = spec.label.to_string();
                let store_for_event = store.clone();
                win.on_window_event(move |event| match event {
                    WindowEvent::Moved(pos) => {
                        store_for_event.set_position(&label_owned, pos.x, pos.y);
                    }
                    WindowEvent::Resized(size) => {
                        store_for_event.set_size(
                            &label_owned,
                            size.width,
                            size.height,
                        );
                    }
                    _ => {}
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_system_stats,
            get_forecast,
            get_widget_locked,
            set_widget_locked,
        ])
        .run(tauri::generate_context!())
        .expect("failed to launch tauri application");
}
