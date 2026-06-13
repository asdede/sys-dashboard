// Typed wrappers around tauri's `invoke()`.
//
// Why bother? It centralises the contract between the Rust commands and
// the frontend. If you change a `#[tauri::command]` signature, you only
// need to update the matching interface here, and TypeScript will then
// point you at every caller that needs to follow.
//
// invoke() under the hood:
//   - Serialises the second argument (a plain object) as JSON.
//   - Posts it over the platform IPC channel to the Tauri runtime.
//   - The runtime routes it to the matching #[tauri::command] function
//     by name, deserialises arguments, awaits the return value, and
//     ships it back as JSON.
//   - Returns a Promise that resolves to the deserialised return value
//     (or rejects with the Err string).

import { invoke } from "@tauri-apps/api/core";

/** Mirrors `SystemStats` in `src-tauri/src/lib.rs`. */
export interface SystemStats {
  cpuPercent: number;
  ramUsedBytes: number;
  ramTotalBytes: number;
  /** null when NVML / NVIDIA driver is unavailable. */
  gpuPercent: number | null;
  vramUsedBytes: number | null;
  vramTotalBytes: number | null;
}

/** Mirrors `DayForecast` in `src-tauri/src/weather/mod.rs`. */
export interface DayForecast {
  label: string;
  condition: string;
  tempHighC: number;
  tempLowC: number;
}

export interface CurrentForecast {
  label: string;
  condition: string;
  tempC: number;
  weekday: string;
}

export interface FutureForecast {
  label: string;
  condition: string;
  tempC: number;
  plusHours: number;
  weekday: string;
}

/** Mirrors `Forecast` in `src-tauri/src/weather/mod.rs`. */
export interface Forecast {
  location: string;
  current: CurrentForecast;
  days: DayForecast[];
  future: FutureForecast[];
}

export function getSystemStats(): Promise<SystemStats> {
  return invoke<SystemStats>("get_system_stats");
}

export function getForecast(): Promise<Forecast> {
  return invoke<Forecast>("get_forecast");
}

/** Read the persisted lock state for one widget window. */
export function getWidgetLocked(label: string): Promise<boolean> {
  return invoke<boolean>("get_widget_locked", { label });
}

/** Persist the new lock state for one widget window. */
export function setWidgetLocked(label: string, locked: boolean): Promise<void> {
  return invoke<void>("set_widget_locked", { label, locked });
}
