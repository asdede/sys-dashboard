# Packaging — autostart install

How to make `sys-dashboard` launch every time you log in, with no entry
in the application launcher.

## 1. Build a release binary

From the repo root:

```bash
npm run tauri build
```

This produces:

- `src-tauri/target/release/sys-dashboard` — the bare binary
- `src-tauri/target/release/bundle/...`    — distro packages (.deb, .rpm,
  AppImage, depending on what Tauri's bundler detected on your system)

For an autostart-only install we use the bare binary.

## 2. Install the binary to a stable location

```bash
mkdir -p ~/.local/bin
install -m 0755 src-tauri/target/release/sys-dashboard ~/.local/bin/sys-dashboard
```

`~/.local/bin` is on the default `PATH` on Fedora / most modern Linux
distros. The binary needs to live somewhere stable because the autostart
desktop file will reference it by absolute path — leaving it inside
`target/release/` means a `cargo clean` would silently break login.

## 3. Install the autostart entry

```bash
mkdir -p ~/.config/autostart
sed "s|__INSTALL_PATH__|$HOME/.local/bin/sys-dashboard|g" \
    packaging/sys-dashboard.desktop \
  > ~/.config/autostart/sys-dashboard.desktop
```

That replaces the `__INSTALL_PATH__` placeholder with the absolute path
to the binary you just installed and drops the result into the standard
[XDG autostart](https://specifications.freedesktop.org/autostart-spec/autostart-spec-latest.html)
directory.

## 4. Verify

Log out and back in. The widgets should appear at the positions they
were in the last time you used them; nothing should appear in the
application launcher or in the dash.

You can also test without logging out:

```bash
env GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 \
    ~/.local/bin/sys-dashboard &
disown
```

`disown` detaches the background job from the shell so closing the
terminal won't kill the widgets.

## 5. Updating

After pulling new code or making local changes, rebuild and re-install
the binary; the autostart file does not need to change because it
references the install path, not the build output:

```bash
npm run tauri build && \
  install -m 0755 src-tauri/target/release/sys-dashboard \
                  ~/.local/bin/sys-dashboard
```

## 6. Uninstalling

```bash
rm ~/.config/autostart/sys-dashboard.desktop
rm ~/.local/bin/sys-dashboard
rm -rf ~/.config/dev.sysdashboard.app
```

The last line also deletes the saved widget positions / sizes.

## Notes on appearance in GNOME

The widgets are configured with `skip_taskbar(true)` and the autostart
entry sets `NoDisplay=true`, so:

- they do **not** show up in the dash / application grid;
- they do **not** show up in the taskbar / Activities favourite area.

They **will** still appear in the GNOME Activities overview (Super key)
because that view explicitly lists every open window and GNOME does not
expose a clean way to opt out from an app. If you want them fully
hidden from the overview as well, install something like the
"Hide Activities" or "Window Calls" extension and configure it to skip
windows whose WM_CLASS matches `sys-dashboard`.

The widgets also persist across desktop / workspace switches because
they are created with `always_on_top(true)`. To pin them to a specific
workspace instead, drop `always_on_top` in `src-tauri/src/lib.rs` and
let your compositor's own "always on visible workspace" toggle do it.
