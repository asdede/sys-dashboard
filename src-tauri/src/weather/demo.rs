//! [`DemoProvider`] - a stand-in [`WeatherProvider`] that returns
//! hardcoded data so the rest of the scaffold can run offline.
//!
//! ## TODO(you): replace this with a real provider
//!
//! Suggested workflow when you wire your country's API:
//!
//!   1. Add HTTP + JSON deps to `src-tauri/Cargo.toml`:
//!      ```toml
//!      reqwest = { version = "0.12", features = ["json", "blocking", "rustls-tls"] }
//!      ```
//!   2. Create a sibling file, e.g. `your_country.rs`, defining a
//!      `YourProvider` struct that implements
//!      [`crate::weather::WeatherProvider`]. Inside `forecast()`:
//!         * `reqwest::blocking::get(...)` your endpoint
//!         * `.json::<YourApiResponse>()?` it (with a serde struct)
//!         * map fields onto [`crate::weather::DayForecast`]
//!   3. In `src/lib.rs`, swap the `Box::new(DemoProvider::default())`
//!      line for `Box::new(your_country::YourProvider::new(...))`.
//!
//! Tip: keep the `forecast()` method **synchronous**. Tauri commands run
//! on a thread pool, and a blocking HTTP call inside is perfectly fine -
//! it avoids the async-runtime gymnastics you'd otherwise need.

use super::{CurrentForecast, DayForecast, FutureForecast, Forecast, WeatherProvider};

#[derive(Default)]
pub struct DemoProvider;

impl WeatherProvider for DemoProvider {
    fn forecast(&self) -> Result<Forecast, String> {
        // Three obviously-fake days that exercise the three main icon
        // paths (clear/cloudy/rain). Add a "snow" day if you want to
        // see the snow icon during development.
        Ok(Forecast {
            location: "Best City Ever".to_string(),
            current: CurrentForecast {
                label: "Now".into(),
                condition: "clear".into(),
                temp_c: 24.0,
                weekday: "Today".into(),
            },

            days: vec![
                DayForecast {
                    label: "Today".into(),
                    condition: "clear".into(),
                    temp_high_c: 24.0,
                    temp_low_c: 14.0,
                },
                DayForecast {
                    label: "Tomorrow".into(),
                    condition: "cloudy".into(),
                    temp_high_c: 21.0,
                    temp_low_c: 13.0,
                },
                DayForecast {
                    label: "Day after".into(),
                    condition: "rain".into(),
                    temp_high_c: 18.0,
                    temp_low_c: 11.0,
                },
            ],
            future: vec![
                FutureForecast {
                    label: "In 2 hours".into(),
                    condition: "clear".into(),
                    temp_c: 24.0,
                    plus_hours: 2,
                    weekday: "Today".into(),
                },
                FutureForecast {
                    label: "In 4 hours".into(),
                    condition: "cloudy".into(),
                    temp_c: 21.0,
                    plus_hours: 4,
                    weekday: "Today".into(),
                },
                FutureForecast {
                    label: "In 6 hours".into(),
                    condition: "rain".into(),
                    temp_c: 18.0,
                    plus_hours: 6,
                    weekday: "Today".into(),
                },
            ],
        })
    }
}
