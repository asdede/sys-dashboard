//! Weather forecast abstraction.
//!
//! The whole point of this module is the [`WeatherProvider`] trait: the
//! rest of the app speaks only the trait, never a concrete provider.
//! That makes swapping in your country's API a one-line change in
//! `lib.rs`.
//!
//! Concrete providers live in sibling files:
//!
//!   * [`demo`] - hardcoded data for the scaffold.
//!   * `your_country` - **you'll add this**.

pub mod demo;

use serde::Serialize;

/// One day of forecast data sent to the frontend as JSON.
///
/// `condition` is intentionally a `String` rather than a Rust enum:
///
///   * It crosses the IPC boundary as plain JSON, where the TS side
///     already keys icons by string. An enum would just add a
///     translation layer.
///   * You can extend the vocabulary (e.g. "fog", "thunderstorm",
///     "mist") without changing this Rust signature - just add a key in
///     `src/forecast.ts`'s ICONS map.
///
/// Recommended values: `"clear" | "cloudy" | "rain" | "snow"`.
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
pub struct Forecast {
    pub location: String,
    pub days: Vec<DayForecast>,
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
