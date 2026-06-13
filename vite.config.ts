// Vite config for the Tauri 2 frontend.
//
// Notes for the curious:
//   - We force a fixed port (1420) because tauri.conf.json's `devUrl`
//     points at it; if Vite picked a different port Tauri would 404 in
//     dev mode.
//   - clearScreen:false keeps Rust compile errors visible alongside Vite
//     output - otherwise Vite repaints the terminal and you lose them.
//   - The TAURI_DEV_HOST env var is set by `tauri dev --host` for testing
//     on real devices over LAN; it is undefined in the common case.

import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      // Don't trigger a frontend reload when Rust source changes - Tauri
      // handles the backend rebuild itself.
      ignored: ["**/src-tauri/**"],
    },
  },
});
