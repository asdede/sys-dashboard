// Cargo build script. Tauri's build helper does several things:
//   - Re-runs whenever tauri.conf.json changes.
//   - Generates JSON schemas for capabilities into src-tauri/gen/.
//   - Embeds the configured app icon into the binary.
//
// We don't need to customise it - the canonical one-line body is enough.
fn main() {
    tauri_build::build()
}
