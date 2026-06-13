# sys-dashboard

## System monitor + weather widged

Movable and rezisable.

On first launch spawns all with default values. Changes to size and position are saved after first edit.
This data is saved to `~/.config/dev.sysdashboard.app/widgets.json`

Includes:
- GPU usage and vram
- Ram usage
- CPU usage
- Weather forecast widged

![img](./img/all.png)

## Configuration

### Ilmatieteenlaitos forecast api (Scandinavian)
- Default api (And currently only one)
- Change the city in widgets.json config

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

## Configuration

Per-widget positions, lock state, and the forecast city all live in
one JSON file under the OS-standard config dir (Linux:
`~/.config/dev.sysdashboard.app/widgets.json`):

```json
{
  "place": "Helsinki",
  "widgets": {
    "cpu":      { "x": 60,  "y": 60,  "width": 120, "height": 140, "locked": false },
    "forecast": { "x": 60,  "y": 220, "width": 260, "height": 260, "locked": false }
  }
}
```

`place` is the town/city name passed straight through to the FMI Open
Data WFS as `place=`. Defaults to `"Helsinki"` if missing.
Restart the app after editing.

## How to extend

### 1. Swap weather providers

The forecast widget is wired to
[`FmiProvider`](src-tauri/src/weather/fmi.rs), which hits the Finnish
Meteorological Institute Open Data WFS using the
`fmi::forecast::edited::weather::scandinavia::point::simple` stored
query. To plug in a different country's API, write a sibling module
that implements [`WeatherProvider`](src-tauri/src/weather/mod.rs) and
swap the `Box::new(FmiProvider::new(...))` line in `src-tauri/src/lib.rs`.

The trait keeps `forecast()` synchronous on purpose: Tauri commands run
on a thread pool, so a blocking HTTP call inside is fine and skips a
lot of async-runtime ceremony.

The original demo provider lives at `src-tauri/src/weather/demo.rs`
and is kept around as an offline fallback.

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
