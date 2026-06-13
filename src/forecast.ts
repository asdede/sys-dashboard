// Forecast renderer.
//
// Given a Forecast from the Rust side, draws three day-cards into a
// container element. Icons are inline SVG kept in a record keyed by
// the `condition` string the Rust side emits.
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

/** Replace `host`'s contents with a card per day in the forecast. */
export function renderForecast(host: HTMLElement, forecast: Forecast): void {
  // We rebuild innerHTML on every refresh. With three cards every 15+
  // minutes that's effectively free. lit-html / morphdom would only be
  // worth the dependency if cards updated many times per second.
  host.innerHTML = forecast.days
    .map((day) => {
      const icon = ICONS[day.condition] ?? FALLBACK_ICON;
      const hi = Math.round(day.tempHighC);
      const lo = Math.round(day.tempLowC);
      // \u00B0 is the degree sign without smuggling a non-ASCII char
      // through the source file.
      return `
        <div class="day" title="${escapeHtml(forecast.location)}">
          <span class="day-label">${escapeHtml(day.label)}</span>
          ${icon}
          <span class="temps"><span class="hi">${hi}\u00B0</span> / <span class="lo">${lo}\u00B0</span></span>
        </div>`;
    })
    .join("");
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
