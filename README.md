# sys-dashboard

A minimal Tauri 2 + TypeScript scaffold for an always-on-top transparent
desktop widget on Fedora 43. Three live circular gauges (CPU, RAM,
NVIDIA GPU/VRAM) plus a 3-day weather forecast (currently a demo stub).

The codebase is intentionally small and heavily commented: every
non-trivial file walks you through the relevant Tauri 2 / `sysinfo` /
NVML / Canvas 2D concept while you read it.

## Layout

```
sys-dashboard/
├── index.html             # vite entry; defines the widget DOM skeleton
├── src/                   # frontend (vanilla TS)
│   ├── main.ts            # bootstraps gauges, polls Rust at 1 Hz
│   ├── api.ts             # typed wrappers around tauri's invoke()
│   ├── gauge.ts           # animated Canvas 2D ring gauge
│   ├── forecast.ts        # 3 day-cards with inline SVG icons
│   └── style.css
└── src-tauri/             # backend (rust)
    ├── Cargo.toml
    ├── tauri.conf.json    # window: transparent, borderless, on-top
    ├── capabilities/      # Tauri 2 permission declarations
    └── src/
        ├── main.rs        # thin entry point
        ├── lib.rs         # commands + AppState
        ├── monitors/
        │   ├── cpu_ram.rs # sysinfo - CPU% + RAM
        │   └── gpu.rs     # nvml-wrapper - GPU util + VRAM
        └── weather/
            ├── mod.rs     # WeatherProvider trait + Forecast types
            └── demo.rs    # hardcoded stand-in (TODO(you): replace)
```

## Prerequisites (Fedora 43)

System libraries Tauri's WebKitGTK backend needs:

```bash
sudo dnf install \
  webkit2gtk4.1-devel \
  openssl-devel \
  curl wget file \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  patchelf
```

For the GPU gauge to light up, install the NVIDIA driver (provides
`libnvidia-ml.so.1`):

```bash
sudo dnf install akmod-nvidia
```

If the driver is missing the GPU gauge silently shows **N/A** - no
crash.

Toolchains:

- Rust 1.77+: <https://rustup.rs/>
- Node 18+

## Run / build

```bash
npm install
npm run tauri dev      # hot-reload dev mode
npm run tauri build    # produces .rpm + AppImage in
                       # src-tauri/target/release/bundle/
```

## How to extend

Two seams are deliberately marked with `// TODO(you):`. Search the repo
for that string and you'll land at the right place.

### 1. Plug in a real weather provider

`src-tauri/src/weather/demo.rs` is a hardcoded `WeatherProvider`. To
replace it with your country's API:

1. Add an HTTP client to `src-tauri/Cargo.toml`:

   ```toml
   reqwest = { version = "0.12", features = ["json", "blocking", "rustls-tls"] }
   ```

2. Create a sibling file, e.g. `src-tauri/src/weather/your_country.rs`,
   with a struct that implements `WeatherProvider`. Inside `forecast()`,
   call your endpoint with `reqwest::blocking::get(...)`, parse the
   JSON with serde, and map fields onto `DayForecast`.

3. In `src-tauri/src/lib.rs`, swap

   ```rust
   weather: Box::new(DemoProvider::default()),
   ```

   for your new provider.

The trait keeps `forecast()` synchronous on purpose: Tauri commands run
on a thread pool, so a blocking HTTP call inside is fine and skips a
lot of async-runtime ceremony.

### 2. Better icons / more weather conditions

The inline-SVG map at the top of `src/forecast.ts` only covers
`clear / cloudy / rain / snow`. You can:

- Add new keys to the same `ICONS` record (e.g. `"fog"`,
  `"thunderstorm"`), then have your provider emit those condition
  strings; or
- Replace the map with a loader for a richer set such as
  [Meteocons](https://bas.dev/work/meteocons) (MIT licensed).

## Notes on "desktop widget" on GNOME / Wayland

Stock Fedora 43 runs GNOME on Wayland, which does **not** implement
`wlr-layer-shell`. There is therefore no API for a window to be pinned
*below* normal app windows. The closest behaviour Tauri can give you on
GNOME is what's configured here: borderless, transparent, always-on-top,
hidden from the taskbar.

A second Wayland quirk worth knowing: clients are not allowed to
**set** their own window position. Mutter decides where each window
opens, so the `x` / `y` values saved in `widgets.json` will be ignored
on the very first launch (every widget pops at Mutter's default spot)
and any post-launch `set_position` call is a no-op too. The user has
to drag the windows where they want them; that placement *does* save
because Mutter still reports the new physical position via
`WindowEvent::Moved`. If you find this annoying, run the app under
XWayland and the positions become authoritative:

```bash
GDK_BACKEND=x11 npm run tauri dev
```

If you ever want a true desktop-anchored widget the alternatives are:

- A **GNOME Shell extension** (different stack: GJS/JavaScript), or
- Switching to a wlroots compositor (sway, Hyprland) where
  layer-shell windows are first-class citizens.

## Optional: autostart

To launch the widget at login, drop a desktop entry into
`~/.config/autostart/`:

```ini
# ~/.config/autostart/sys-dashboard.desktop
[Desktop Entry]
Type=Application
Name=sys-dashboard
Exec=/path/to/sys-dashboard
X-GNOME-Autostart-enabled=true
```
