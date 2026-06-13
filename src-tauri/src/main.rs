// Thin binary entry point.
//
// In Tauri 2 the convention is to keep almost all code in the library
// crate (src/lib.rs) and have main.rs just call into it. Two reasons:
//
//   1. Mobile targets (Android/iOS) compile the library as a cdylib and
//      do not use main.rs at all. Keeping logic in lib.rs means the
//      same code path runs on every platform.
//   2. Integration tests can `use sys_dashboard_lib::*;` only when the
//      logic lives in a library crate.
//
// On Windows release builds you would normally add
// `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
// here to suppress the console window. We omit it: this scaffold
// targets Linux only.
fn main() {
    // Force the GTK X11 backend (XWayland on Wayland sessions). The
    // widget UX assumes three things that native Wayland's
    // xdg_toplevel deliberately does NOT support:
    //
    //   * the app reading its own absolute window position
    //   * the app moving its own window to a remembered position
    //   * "always on top" with arbitrary z-order placement
    //
    // Without this hint, on a Wayland session WindowEvent::Moved fires
    // with (0, 0) and set_position() is a silent no-op, so saved
    // positions cannot be restored - widgets pile up in the top-left
    // corner on every launch. Forcing the X11 backend routes through
    // XWayland, which honours all three operations.
    //
    // Must be set BEFORE the windowing layer is touched, hence here
    // at the very top of main() rather than inside run(). Safe on
    // Rust 2021 edition (set_var moves to `unsafe` in edition 2024;
    // see Cargo.toml `edition = "2021"`).
    std::env::set_var("GDK_BACKEND", "x11");

    sys_dashboard_lib::run();
}
