# sys-dashboard

## System monitor + weather widged

Movable and rezisable.

Includes:
- GPU usage and vram
- Ram usage
- CPU usage
- Weather forecast widged

![img](./img/all.png)


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
