// Forecast renderer.
//
// Given a Forecast from the Rust side, draws four stacked sections into
// a container element:
//
//   1. Header   - location + today's date
//   2. Current  - large icon + current temperature
//   3. Hourly   - flat row of +2h / +4h / +6h, no card backgrounds
//   4. Daily    - row of boxed cards (today, tomorrow, ...)
//
// Icons are inline SVG kept in a record keyed by the `condition` string
// the Rust side emits.
//
// Why inline SVG instead of <img src="...">?
//   - One bundle, no extra HTTP requests in dev or runtime.
//   - `currentColor` lets the icon inherit its colour from CSS, so we
//     can re-theme everything by changing one variable.
//
// TODO(you): expand ICONS once your real provider produces more
// conditions (fog, thunderstorm, mist, ...) - or replace this map with
// a loader for a richer icon set such as Meteocons.

import type { Forecast } from "./api";

const ICONS: Record<string, string> = {
  // Each SVG uses a 28x28 viewBox and `currentColor`, so a parent CSS
  // color rule sets every icon at once.
  clear: `
    <svg viewBox="0 0 28 28" stroke="currentColor" fill="none" stroke-width="1.6" stroke-linecap="round">
      <circle cx="14" cy="14" r="4" fill="currentColor" stroke="none"/>
      <line x1="14" y1="3"  x2="14" y2="6"/>
      <line x1="14" y1="22" x2="14" y2="25"/>
      <line x1="3"  y1="14" x2="6"  y2="14"/>
      <line x1="22" y1="14" x2="25" y2="14"/>
      <line x1="6.2"  y1="6.2"  x2="8.2"  y2="8.2"/>
      <line x1="19.8" y1="19.8" x2="21.8" y2="21.8"/>
      <line x1="6.2"  y1="21.8" x2="8.2"  y2="19.8"/>
      <line x1="19.8" y1="8.2"  x2="21.8" y2="6.2"/>
    </svg>`,

  cloudy: `
    <svg viewBox="0 0 28 28" stroke="currentColor" fill="currentColor" stroke-width="1.2" stroke-linejoin="round">
      <path d="M9 19 H21 a4 4 0 0 0 0.4 -7.96 A6 6 0 0 0 9.6 10.5 A4.5 4.5 0 0 0 9 19 z"/>
    </svg>`,

  rain: `
    <svg viewBox="0 0 28 28" stroke="currentColor" fill="none" stroke-width="1.4" stroke-linecap="round">
      <path d="M9 14 H21 a4 4 0 0 0 0.4 -7.96 A6 6 0 0 0 9.6 5.5 A4.5 4.5 0 0 0 9 14 z" fill="currentColor"/>
      <line x1="10" y1="18" x2="9"  y2="22"/>
      <line x1="15" y1="18" x2="14" y2="22"/>
      <line x1="20" y1="18" x2="19" y2="22"/>
    </svg>`,

  snow: `
    <svg viewBox="0 0 28 28" stroke="currentColor" fill="none" stroke-width="1.2" stroke-linecap="round">
      <path d="M9 14 H21 a4 4 0 0 0 0.4 -7.96 A6 6 0 0 0 9.6 5.5 A4.5 4.5 0 0 0 9 14 z" fill="currentColor"/>
      <circle cx="10" cy="20" r="0.9" fill="currentColor" stroke="none"/>
      <circle cx="14" cy="22" r="0.9" fill="currentColor" stroke="none"/>
      <circle cx="18" cy="20" r="0.9" fill="currentColor" stroke="none"/>
    </svg>`,
};

const FALLBACK_ICON = `
  <svg viewBox="0 0 28 28" stroke="currentColor" fill="none" stroke-width="1.4">
    <circle cx="14" cy="14" r="9"/>
    <text x="14" y="18" text-anchor="middle" font-size="11" fill="currentColor" stroke="none">?</text>
  </svg>`;

const WEEKDAY_NAMES = [
  "Sunday",
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
] as const;

const MONTH_NAMES = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun",
  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
] as const;

/** Replace `host`'s contents with the four-section forecast layout. */
export function renderForecast(host: HTMLElement, forecast: Forecast): void {
  // We rebuild innerHTML on every refresh. With a handful of nodes every
  // 15+ minutes that's effectively free. lit-html / morphdom would only
  // be worth the dependency if the strip updated many times per second.

  const now = new Date();
  const dateText = `${WEEKDAY_NAMES[now.getDay()]} \u00B7 ${now.getDate()} ${MONTH_NAMES[now.getMonth()]} ${now.getFullYear()}`;

  const currentIcon = ICONS[forecast.current.condition] ?? FALLBACK_ICON;
  const currentTemp = Math.round(forecast.current.tempC);

  const hourly = forecast.future
    .map((f) => {
      const icon = ICONS[f.condition] ?? FALLBACK_ICON;
      const t = Math.round(f.tempC);
      return `
        <div class="hour">
          <span class="hour-label">+${f.plusHours}h</span>
          ${icon}
          <span class="hour-temp">${t}\u00B0</span>
        </div>`;
    })
    .join("");

  const days = forecast.days
    .map((day) => {
      const icon = ICONS[day.condition] ?? FALLBACK_ICON;
      const hi = Math.round(day.tempHighC);
      const lo = Math.round(day.tempLowC);
      // \u00B0 is the degree sign without smuggling a non-ASCII char
      // through the source file.
      return `
        <div class="day">
          <span class="day-label">${escapeHtml(day.label)}</span>
          ${icon}
          <span class="temps"><span class="hi">${hi}\u00B0</span> / <span class="lo">${lo}\u00B0</span></span>
        </div>`;
    })
    .join("");

  host.innerHTML = `
    <header class="forecast-header" title="${escapeHtml(forecast.location)}">
      <h2 class="forecast-title">${escapeHtml(forecast.location.toUpperCase())}</h2>
      <div class="forecast-date">${escapeHtml(dateText)}</div>
    </header>
    <div class="current-weather">
      <div class="current-icon">${currentIcon}</div>
      <div class="current-meta">
        <div class="current-temp">${currentTemp}\u00B0</div>
        <div class="current-condition">${escapeHtml(forecast.current.label)} \u00B7 ${escapeHtml(forecast.current.condition)}</div>
      </div>
    </div>
    <div class="future-weather">${hourly}</div>
    <div class="days-weather">${days}</div>`;
}

// Minimal HTML escaper - good enough for the labels we control on the
// Rust side, and a sensible habit when concatenating strings into
// innerHTML.
function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => {
    const map: Record<string, string> = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#39;",
    };
    return map[c]!;
  });
}
