//! Finnish Meteorological Institute Open Data WFS provider.
//!
//! ## What this file is
//!
//! A [`WeatherProvider`] that turns one HTTP GET against
//! `https://opendata.fmi.fi/wfs` into the [`Forecast`] shape the
//! frontend expects: a "now" point, three short-horizon hourly
//! forecasts (`+2 h / +4 h / +6 h`), and three daily min-max summaries
//! (today / tomorrow / day after).
//!
//! ## How the WFS endpoint works
//!
//! WFS is a GIS protocol, not REST. The interesting data is hidden
//! behind named *stored queries*; we use one of them:
//!
//!   `fmi::forecast::edited::weather::scandinavia::point::simple`
//!
//! It is the forecaster-edited "best official" mix - smooth, hourly
//! out to ~66 h, then 3-hourly out to ~10 days. Querying it with
//! `place=<town>` resolves the place name to coordinates server-side,
//! and `parameters=Temperature,WeatherSymbol3` keeps the payload tiny.
//!
//! The `::simple` family returns a flat `BsWfs:BsWfsElement` per
//! `(time × parameter)` cell, which is trivially streamable.
//!
//! ## Why this is synchronous
//!
//! Tauri commands already run on a thread pool, so a blocking HTTP
//! call inside `forecast()` is fine and lets us avoid pulling tokio
//! into the dependency graph just for one GET.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use chrono_tz::Europe::Helsinki;
use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::config::ConfigStore;
use crate::weather::{
    CurrentForecast, DayForecast, Forecast, FutureForecast, WeatherProvider,
};

const ENDPOINT: &str = "https://opendata.fmi.fi/wfs";
const STORED_QUERY: &str =
    "fmi::forecast::edited::weather::scandinavia::point::simple";
/// How far ahead we ask the server for. Needs to cover "day after" in
/// Helsinki local time (~71 h) plus a small margin so a late-night
/// query still has data through end-of-day.
const HORIZON_HOURS: i64 = 84;
const REQUEST_TIMEOUT: StdDuration = StdDuration::from_secs(15);

/// Provider that reads its target town/city from the shared
/// [`ConfigStore`] on every call, so editing `widgets.json` and
/// triggering a forecast refresh on the frontend is enough to switch
/// cities without restarting the app.
pub struct FmiProvider {
    config: Arc<ConfigStore>,
    http: reqwest::blocking::Client,
}

impl FmiProvider {
    pub fn new(config: Arc<ConfigStore>) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("sys-dashboard/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest blocking client");
        Self { config, http }
    }

    fn fetch_xml(
        &self,
        place: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<String, String> {
        // FMI accepts ISO-8601 timestamps with a trailing `Z`.
        let starttime = start.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let endtime = end.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let response = self
            .http
            .get(ENDPOINT)
            .query(&[
                ("service", "WFS"),
                ("version", "2.0.0"),
                ("request", "getFeature"),
                ("storedquery_id", STORED_QUERY),
                ("place", place),
                ("parameters", "Temperature,WeatherSymbol3"),
                ("timestep", "60"),
                ("starttime", &starttime),
                ("endtime", &endtime),
            ])
            .send()
            .map_err(|e| format!("FMI request failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("FMI returned HTTP {}", response.status()));
        }
        response
            .text()
            .map_err(|e| format!("FMI body read failed: {e}"))
    }
}

impl WeatherProvider for FmiProvider {
    fn forecast(&self) -> Result<Forecast, String> {
        let place = self.config.place();
        let now = Utc::now();
        // We pull from `now - 1 h` so the server has at least one row
        // strictly before "now" to clamp `current` against. The server
        // automatically clips to the latest available forecast issue.
        let start = now - Duration::hours(1);
        let end = now + Duration::hours(HORIZON_HOURS);

        let xml = self.fetch_xml(&place, start, end)?;
        let rows = parse_simple_features(&xml)?;
        if rows.is_empty() {
            return Err(format!(
                "FMI returned no rows for '{place}' - is the place name spelled correctly?"
            ));
        }

        build_forecast(&place, now, &rows)
    }
}

/// One `(time, parameter, value)` triple, which is exactly one
/// `BsWfs:BsWfsElement` from the `simple` feature shape.
#[derive(Debug)]
struct Row {
    time: DateTime<Utc>,
    name: String,
    value: f32,
}

/// Stream-parse the WFS XML into rows. We recognise three child
/// elements inside each `BsWfsElement`:
///
///   * `Time`            - RFC-3339 UTC timestamp.
///   * `ParameterName`   - "Temperature" or "WeatherSymbol3".
///   * `ParameterValue`  - decimal, possibly "NaN".
///
/// Everything else (location, schema noise, namespace declarations) is
/// ignored. Robust against unknown wrapper elements because we only
/// react to local names we recognise.
fn parse_simple_features(xml: &str) -> Result<Vec<Row>, String> {
    let mut reader = Reader::from_str(xml);

    enum Field {
        None,
        Time,
        Name,
        Value,
    }
    let mut current = Field::None;
    let mut time: Option<DateTime<Utc>> = None;
    let mut name: Option<String> = None;
    let mut value: Option<f32> = None;
    let mut rows: Vec<Row> = Vec::new();
    let mut buf: Vec<u8> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(format!(
                    "XML parse error at byte {}: {e}",
                    reader.buffer_position()
                ))
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let local = local_name(e.name().as_ref())?;
                match local.as_str() {
                    "BsWfsElement" => {
                        time = None;
                        name = None;
                        value = None;
                    }
                    "Time" => current = Field::Time,
                    "ParameterName" => current = Field::Name,
                    "ParameterValue" => current = Field::Value,
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let local = local_name(e.name().as_ref())?;
                match local.as_str() {
                    "Time" | "ParameterName" | "ParameterValue" => {
                        current = Field::None;
                    }
                    "BsWfsElement" => {
                        if let (Some(t), Some(n), Some(v)) =
                            (time.take(), name.take(), value.take())
                        {
                            rows.push(Row { time: t, name: n, value: v });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(text)) => {
                let raw = text.unescape().map_err(|e| e.to_string())?;
                let s = raw.trim();
                if s.is_empty() {
                    continue;
                }
                match current {
                    Field::Time => {
                        time = DateTime::parse_from_rfc3339(s)
                            .ok()
                            .map(|t| t.with_timezone(&Utc));
                    }
                    Field::Name => name = Some(s.to_owned()),
                    Field::Value => {
                        // FMI uses the literal "NaN" for missing values.
                        if !s.eq_ignore_ascii_case("NaN") {
                            value = s.parse().ok();
                        }
                    }
                    Field::None => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(rows)
}

fn local_name(qname: &[u8]) -> Result<String, String> {
    // qname is "prefix:local" or just "local". Strip everything up to
    // and including the colon; we never disambiguate by namespace
    // because the FMI schema gives each tag a unique local name.
    let bytes = match qname.iter().position(|b| *b == b':') {
        Some(i) => &qname[i + 1..],
        None => qname,
    };
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|e| format!("non-UTF8 element name: {e}"))
}

/// Bucket rows by timestamp into `(temperature, symbol)` pairs and
/// project them onto the `Forecast` shape the frontend understands.
fn build_forecast(
    place: &str,
    now: DateTime<Utc>,
    rows: &[Row],
) -> Result<Forecast, String> {
    // BTreeMap keeps timestamps sorted so the closest-to-target search
    // below remains O(n) and predictable, and so day grouping yields
    // chronological iteration.
    let mut by_time: BTreeMap<DateTime<Utc>, Bucket> = BTreeMap::new();
    for row in rows {
        let entry = by_time.entry(row.time).or_default();
        match row.name.as_str() {
            "Temperature" => entry.temp = Some(row.value),
            "WeatherSymbol3" => entry.symbol = Some(row.value),
            _ => {}
        }
    }

    if by_time.is_empty() {
        return Err("no usable forecast rows".into());
    }

    // Find the row whose timestamp is closest (in either direction) to
    // `target`. Since `by_time` is small (~80 rows over 84 hours hourly)
    // a linear scan is fine and dodges the off-by-one-vs-`range`
    // gymnastics a smarter search would need.
    let closest_to = |target: DateTime<Utc>| -> Option<(DateTime<Utc>, Bucket)> {
        by_time
            .iter()
            .min_by_key(|(t, _)| t.signed_duration_since(target).num_seconds().abs())
            .map(|(t, b)| (*t, *b))
    };

    let (current_t, current_bucket) =
        closest_to(now).ok_or("no rows close to 'now'")?;
    let current = CurrentForecast {
        label: "Now".into(),
        condition: symbol_to_condition(current_bucket.symbol),
        temp_c: current_bucket
            .temp
            .ok_or("Temperature missing for current row")?,
        weekday: weekday_label_local(&current_t),
    };

    let mut future: Vec<FutureForecast> = Vec::with_capacity(3);
    for &h in &[2u32, 4, 6] {
        let target = now + Duration::hours(h as i64);
        if let Some((t, bucket)) = closest_to(target) {
            // If the server happens to omit Temperature for this hour,
            // skip the slot rather than emit a placeholder; the UI
            // already tolerates a short `future` list.
            let Some(temp) = bucket.temp else { continue };
            future.push(FutureForecast {
                label: format!("In {h} hours"),
                condition: symbol_to_condition(bucket.symbol),
                temp_c: temp,
                plus_hours: h,
                weekday: weekday_label_local(&t),
            });
        }
    }

    let now_local_date = now.with_timezone(&Helsinki).date_naive();
    let day_targets: [(i64, &str); 3] =
        [(0, "Today"), (1, "Tomorrow"), (2, "Day after")];
    let mut days: Vec<DayForecast> = Vec::with_capacity(3);

    for (offset, label) in day_targets {
        let target_date = now_local_date + Duration::days(offset);

        // Local noon, mapped to UTC, is the canonical timestamp we
        // pick a day's representative icon at - matches what every
        // other Finnish weather app shows.
        let noon_utc = match Helsinki
            .with_ymd_and_hms(
                target_date.year(),
                target_date.month(),
                target_date.day(),
                12,
                0,
                0,
            )
            .single()
        {
            Some(t) => t.with_timezone(&Utc),
            // Ambiguous local noon is impossible in Europe/Helsinki
            // (the DST transitions happen at 03:00 / 04:00 local), but
            // play it safe and skip the day rather than panic.
            None => continue,
        };

        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut midday_symbol: Option<(i64, f32)> = None;

        for (t, bucket) in by_time.iter() {
            let local_date = t.with_timezone(&Helsinki).date_naive();
            if local_date != target_date {
                continue;
            }
            if let Some(temp) = bucket.temp {
                if temp < min {
                    min = temp;
                }
                if temp > max {
                    max = temp;
                }
            }
            if let Some(sym) = bucket.symbol {
                let dist = t.signed_duration_since(noon_utc).num_seconds().abs();
                if midday_symbol.map_or(true, |(prev_dist, _)| dist < prev_dist) {
                    midday_symbol = Some((dist, sym));
                }
            }
        }

        if !min.is_finite() || !max.is_finite() {
            // Forecast horizon doesn't reach this day - skip the slot
            // rather than emit obviously-wrong numbers.
            continue;
        }

        days.push(DayForecast {
            label: label.into(),
            condition: symbol_to_condition(midday_symbol.map(|(_, s)| s)),
            temp_high_c: max,
            temp_low_c: min,
        });
    }

    Ok(Forecast {
        location: titlecase(place),
        current,
        days,
        future,
    })
}

#[derive(Default, Clone, Copy)]
struct Bucket {
    temp: Option<f32>,
    symbol: Option<f32>,
}

fn weekday_label_local(t: &DateTime<Utc>) -> String {
    use chrono::Weekday::*;
    match t.with_timezone(&Helsinki).weekday() {
        Mon => "Mon",
        Tue => "Tue",
        Wed => "Wed",
        Thu => "Thu",
        Fri => "Fri",
        Sat => "Sat",
        Sun => "Sun",
    }
    .to_owned()
}

/// Map an FMI `WeatherSymbol3` numeric code into one of the eight
/// condition keys the frontend understands:
///
///   `clear` | `partly-cloudy` | `cloudy` | `fog`
///   `rain`  | `thunder`       | `sleet`  | `snow`
///
/// Code groups (FMI):
///   * `1`              clear
///   * `2`              partly cloudy
///   * `3`              cloudy / overcast
///   * `21..=23`        weak / moderate / strong rain showers
///   * `31..=33`        weak / moderate / strong rain
///   * `41..=43`        weak / moderate / strong snow showers
///   * `51..=53`        weak / moderate / strong snowfall
///   * `61..=63`        weak / moderate / strong thunder
///   * `71..=73`        weak / moderate / strong sleet showers
///   * `81..=83`        weak / moderate / strong sleet
///   * `91`, `92`       mist, fog
///
/// Sleet (a snow / rain mix) keeps its own bucket because the visual
/// signal is meaningfully different from plain rain. Thunder likewise
/// gets its own icon since it's the dramatic one users actually want
/// to notice. Mist and fog both collapse into `fog` because at the
/// widget's icon size the difference isn't legible.
///
/// Falls back to `"cloudy"` for missing or unknown codes - showing a
/// neutral icon is preferable to crashing or to silently substituting
/// "clear" on a rainy day.
fn symbol_to_condition(symbol: Option<f32>) -> String {
    let Some(value) = symbol else {
        return "cloudy".into();
    };
    let code = value.round() as i32;
    let key = match code {
        1 => "clear",
        2 => "partly-cloudy",
        3 => "cloudy",
        21..=23 | 31..=33 => "rain",
        41..=43 | 51..=53 => "snow",
        61..=63 => "thunder",
        71..=73 | 81..=83 => "sleet",
        91 | 92 => "fog",
        _ => "cloudy",
    };
    key.to_owned()
}

/// Capitalise the first character of `s`, leaving the rest as-is.
/// Suitable for Finnish town names, which are conventionally written
/// with one initial capital ("Kajaani", "Helsinki", "Jyväskylä").
fn titlecase(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_groups() {
        assert_eq!(symbol_to_condition(Some(1.0)), "clear");
        assert_eq!(symbol_to_condition(Some(2.0)), "partly-cloudy");
        assert_eq!(symbol_to_condition(Some(3.0)), "cloudy");
        assert_eq!(symbol_to_condition(Some(22.0)), "rain");
        assert_eq!(symbol_to_condition(Some(32.0)), "rain");
        assert_eq!(symbol_to_condition(Some(42.0)), "snow");
        assert_eq!(symbol_to_condition(Some(52.0)), "snow");
        assert_eq!(symbol_to_condition(Some(61.0)), "thunder");
        assert_eq!(symbol_to_condition(Some(63.0)), "thunder");
        assert_eq!(symbol_to_condition(Some(72.0)), "sleet");
        assert_eq!(symbol_to_condition(Some(82.0)), "sleet");
        assert_eq!(symbol_to_condition(Some(91.0)), "fog");
        assert_eq!(symbol_to_condition(Some(92.0)), "fog");

        // Defensive fallbacks.
        assert_eq!(symbol_to_condition(None), "cloudy");
        assert_eq!(symbol_to_condition(Some(999.0)), "cloudy");
    }

    #[test]
    fn parses_a_minimal_simple_feature_collection() {
        // Hand-trimmed mock of one BsWfsElement record. Whitespace is
        // representative of what FMI returns.
        let xml = r#"<?xml version="1.0"?>
<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/2.0"
                       xmlns:gml="http://www.opengis.net/gml/3.2"
                       xmlns:BsWfs="http://xml.fmi.fi/schema/wfs/2.0">
  <wfs:member>
    <BsWfs:BsWfsElement>
      <BsWfs:Location>
        <gml:Point><gml:pos>64.22 27.72</gml:pos></gml:Point>
      </BsWfs:Location>
      <BsWfs:Time>2026-06-14T12:00:00Z</BsWfs:Time>
      <BsWfs:ParameterName>Temperature</BsWfs:ParameterName>
      <BsWfs:ParameterValue>15.5</BsWfs:ParameterValue>
    </BsWfs:BsWfsElement>
  </wfs:member>
  <wfs:member>
    <BsWfs:BsWfsElement>
      <BsWfs:Time>2026-06-14T12:00:00Z</BsWfs:Time>
      <BsWfs:ParameterName>WeatherSymbol3</BsWfs:ParameterName>
      <BsWfs:ParameterValue>3.0</BsWfs:ParameterValue>
    </BsWfs:BsWfsElement>
  </wfs:member>
</wfs:FeatureCollection>"#;

        let rows = parse_simple_features(xml).expect("parse");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "Temperature");
        assert!((rows[0].value - 15.5).abs() < 1e-3);
        assert_eq!(rows[1].name, "WeatherSymbol3");
    }

    #[test]
    fn parses_nan_as_missing() {
        let xml = r#"<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/2.0"
                                            xmlns:BsWfs="http://xml.fmi.fi/schema/wfs/2.0">
  <wfs:member>
    <BsWfs:BsWfsElement>
      <BsWfs:Time>2026-06-14T12:00:00Z</BsWfs:Time>
      <BsWfs:ParameterName>Temperature</BsWfs:ParameterName>
      <BsWfs:ParameterValue>NaN</BsWfs:ParameterValue>
    </BsWfs:BsWfsElement>
  </wfs:member>
</wfs:FeatureCollection>"#;
        let rows = parse_simple_features(xml).expect("parse");
        assert!(rows.is_empty(), "NaN rows should be dropped, got {rows:?}");
    }
}
