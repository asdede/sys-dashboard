//! Weather forecast abstraction.
//!
//! The whole point of this module is the [`WeatherProvider`] trait: the
//! rest of the app speaks only the trait, never a concrete provider.
//! That makes swapping in your country's API a one-line change in
//! `lib.rs`.
//!
//! Concrete providers live in sibling files:
//!
//!   * [`demo`] - hardcoded data, kept around for offline development
//!     and as a fallback you can swap back in when the network is
//!     unreachable.
//!   * [`fmi`] - the real provider used by `lib.rs`. Hits the Finnish
//!     Meteorological Institute Open Data WFS for any Finnish town.

pub mod demo;
pub mod fmi;

use serde::Serialize;

/// One day of forecast data sent to the frontend as JSON.
///
/// `condition` is intentionally a `String` rather than a Rust enum:
///
///   * It crosses the IPC boundary as plain JSON, where the TS side
///     already keys icons by string. An enum would just add a
///     translation layer.
///   * You can extend the vocabulary without changing this Rust
///     signature - just add a key in `src/forecast.ts`'s ICONS map.
///
/// Recommended values: `"clear" | "partly-cloudy" | "cloudy" | "fog"
///                    | "rain" | "thunder" | "sleet" | "snow"`.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DayForecast {
    /// Human-readable label - e.g. "Today", "Tomorrow", "Mon".
    pub label: String,
    /// One of the recommended condition keys above.
    pub condition: String,
    pub temp_high_c: f32,
    pub temp_low_c: f32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CurrentForecast {
    pub label: String,
    pub condition: String,
    pub temp_c: f32,
    pub weekday:  String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FutureForecast {
    pub label: String,
    pub condition: String,
    pub temp_c: f32,
    pub plus_hours: u32,
    pub weekday: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Forecast {
    pub location: String,
    pub current: CurrentForecast,
    pub days: Vec<DayForecast>,
    pub future: Vec<FutureForecast>,
}

/// The seam every alternative weather provider plugs into.
///
/// Implementation notes:
///
///   * `Send + Sync` are required because we store
///     `Box<dyn WeatherProvider>` inside Tauri's app state, which is
///     shared across a worker thread pool.
///   * Returning `Result<_, String>` keeps the trait *object-safe*
///     (no associated error type) and matches the way Tauri commands
///     surface errors to JavaScript.
pub trait WeatherProvider: Send + Sync {
    fn forecast(&self) -> Result<Forecast, String>;
}
