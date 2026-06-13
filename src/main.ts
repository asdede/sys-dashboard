// Frontend shell.
//
// The same bundle is loaded by every widget window. The window's URL
// carries a `?widget=` query that tells this script which of the four
// supported widgets it should render:
//
//   ?widget=cpu        -> CPU% gauge
//   ?widget=ram        -> RAM% gauge with "used / total" subtitle
//   ?widget=gpu        -> VRAM% gauge with "used + compute%" subtitle
//   ?widget=forecast   -> 3-day forecast strip
//
// Each window also gets a "chrome" wrapper - a slim drag handle on top
// plus a lock toggle in the top-right corner that only fades in on
// hover. Toggling the lock removes/adds `data-tauri-drag-region` on
// the handle and persists the new state to a JSON config via the Rust
// side, so it survives restarts.

import "./style.css";

import { getCurrentWindow } from "@tauri-apps/api/window";

import { Gauge } from "./gauge";
import { renderForecast } from "./forecast";
import {
  getForecast,
  getSystemStats,
  getWidgetLocked,
  setWidgetLocked,
} from "./api";

const SAMPLE_INTERVAL_MS = 1000;

type WidgetName = "cpu" | "ram" | "gpu" | "forecast";

const WIDGETS: readonly WidgetName[] = ["cpu", "ram", "gpu", "forecast"];

function readWidget(): WidgetName {
  const raw = new URLSearchParams(location.search).get("widget");
  return (WIDGETS as readonly string[]).includes(raw ?? "")
    ? (raw as WidgetName)
    : "cpu";
}

const widget = readWidget();
document.body.classList.add(`widget-${widget}`);

const root = document.getElementById("app");
if (!root) throw new Error("#app missing in index.html");

/** Two-glyph SVG set for the lock toggle, both 14x14, single colour
 *  via `currentColor` so CSS can re-theme them. */
const LOCK_ICON_LOCKED = `
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
       stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <rect x="4" y="10" width="16" height="11" rx="2"/>
    <path d="M8 10 V7 a4 4 0 0 1 8 0 v3"/>
  </svg>`;

const LOCK_ICON_OPEN = `
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
       stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <rect x="4" y="10" width="16" height="11" rx="2"/>
    <path d="M8 10 V7 a4 4 0 0 1 7.5 -2"/>
  </svg>`;

/** Tiny inline SVG for the bottom-right resize grip. Three short
 *  diagonal strokes hugging the corner, drawn with `currentColor` so
 *  the lock-toggle palette applies. */
const RESIZE_GRIP_ICON = `
  <svg viewBox="0 0 12 12" fill="none" stroke="currentColor"
       stroke-width="1.2" stroke-linecap="round" aria-hidden="true">
    <line x1="11" y1="3"  x2="3"  y2="11"/>
    <line x1="11" y1="6"  x2="6"  y2="11"/>
    <line x1="11" y1="9"  x2="9"  y2="11"/>
  </svg>`;

/** Build the per-window chrome (drag overlay + hover lock + corner
 *  resize grip + content slot) and return the inner content element so
 *  the widget code can mount into it without caring about the chrome.
 *
 *  Why an overlay div instead of marking the canvas draggable? Tauri's
 *  built-in `data-tauri-drag-region` handler checks `event.target` only
 *  (not its ancestors), so a parent attribute would NOT trigger a drag
 *  for clicks on the gauge canvas or a day-card. A transparent overlay
 *  sitting on top of the content gives us one element that's always
 *  the click target. The lock button and resize grip are stacked above
 *  the overlay so their clicks reach them instead.
 *
 *  Why call `startDragging()` ourselves instead of relying on
 *  `data-tauri-drag-region`? On Wayland (Mutter / GNOME) the
 *  compositor demands that `xdg_toplevel.move` be paired with a fresh
 *  pointer input-serial - if we let Tauri marshal an async IPC first,
 *  the serial often expires and Mutter silently refuses to move the
 *  window. Calling `startDragging()` synchronously inside the
 *  mousedown handler (and NOT awaiting the returned Promise) keeps
 *  the serial fresh, which is what makes the drag actually start on
 *  Linux desktops.
 */
function mountChrome(initialLocked: boolean): HTMLElement {
  root!.innerHTML = `
    <div class="frame">
      <div class="content"></div>
      <div class="drag-overlay"></div>
      <button class="lock-toggle" type="button" aria-pressed="${initialLocked}"
              title="${initialLocked ? "Unlock" : "Lock"} position">
      </button>
      <div class="resize-grip" role="button" aria-label="Resize"
           title="Drag to resize">
      </div>
    </div>`;

  const overlay = root!.querySelector(".drag-overlay") as HTMLElement;
  const button = root!.querySelector(".lock-toggle") as HTMLButtonElement;
  const grip = root!.querySelector(".resize-grip") as HTMLElement;
  const content = root!.querySelector(".content") as HTMLElement;

  grip.innerHTML = RESIZE_GRIP_ICON;

  let locked = initialLocked;
  const tauriWindow = getCurrentWindow();

  // Explicit drag: fire-and-forget. Awaiting would let the input
  // serial expire before the compositor's `move` request arrives.
  overlay.addEventListener("mousedown", (e) => {
    if (locked || e.button !== 0) return;
    e.preventDefault();
    tauriWindow.startDragging().catch((err) => {
      console.error("[drag] startDragging failed", err);
    });
  });

  // Resize is also explicit. Stopping propagation prevents the drag
  // handler above from firing first if the grip ever overlaps the
  // overlay's hit region.
  grip.addEventListener("mousedown", (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    tauriWindow.startResizeDragging("SouthEast").catch((err) => {
      console.error("[resize] startResizeDragging failed", err);
    });
  });

  const render = () => {
    button.innerHTML = locked ? LOCK_ICON_LOCKED : LOCK_ICON_OPEN;
    button.title = `${locked ? "Unlock" : "Lock"} position`;
    button.setAttribute("aria-pressed", String(locked));
    document.body.classList.toggle("locked", locked);
  };
  render();

  button.addEventListener("click", async () => {
    locked = !locked;
    render();
    try {

      if (locked) {
        grip.innerHTML = "";
      } else {
        grip.innerHTML = RESIZE_GRIP_ICON;
      }

      await setWidgetLocked(widget, locked);
    } catch (e) {
      // Roll back the optimistic toggle on failure so the UI and the
      // persisted state can never disagree.
      console.error("[lock] set_widget_locked failed", e);
      locked = !locked;
      render();
    }
  });

  return content;
}

/** Format bytes as a short, human label (e.g. "6.2 GB", "512 MB"). */
function formatBytes(bytes: number): string {
  const gb = bytes / 1024 ** 3;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  const mb = bytes / 1024 ** 2;
  return `${Math.round(mb)} MB`;
}

/** Mount one of the three system gauges and start the 1-Hz sample loop. */
function mountSystemGauge(
  content: HTMLElement,
  which: "cpu" | "ram" | "gpu",
): void {
  const color = { cpu: "#5fb3ff", ram: "#7ce0a3", gpu: "#c98aff" }[which];

  // The Gauge mounts an <svg> inside this div - see `gauge.ts` for
  // why we render via SVG instead of <canvas> on this platform.
  content.innerHTML = `
    <div class="gauge"></div>
    <span class="label">${which.toUpperCase()}</span>`;
  const container = content.querySelector(".gauge") as HTMLDivElement;
  const gauge = new Gauge(container, { color });

  const tick = async (): Promise<void> => {
    try {
      const stats = await getSystemStats();
      if (which === "cpu") {
        gauge.setValue(stats.cpuPercent);
      } else if (which === "ram") {
        const pct = stats.ramTotalBytes
          ? (stats.ramUsedBytes / stats.ramTotalBytes) * 100
          : 0;
        gauge.setValue(pct);
        gauge.setSubtitle(
          `${formatBytes(stats.ramUsedBytes)} / ${formatBytes(stats.ramTotalBytes)}`,
        );
      } else {
        if (
          stats.gpuPercent != null &&
          stats.vramUsedBytes != null &&
          stats.vramTotalBytes != null
        ) {
          gauge.setDisabled(false);
          // Ring shows VRAM%, subtitle shows GPU compute%.
          const vramPct = stats.vramTotalBytes
            ? (stats.vramUsedBytes / stats.vramTotalBytes) * 100
            : 0;
          gauge.setValue(vramPct);
          gauge.setSubtitle(
            `${formatBytes(stats.vramUsedBytes)} \u2022 ${Math.round(stats.gpuPercent)}%`,
          );
        } else {
          gauge.setDisabled(true);
          gauge.setSubtitle("no NVML");
        }
      }
    } catch (e) {
      console.error(`[tick:${which}] get_system_stats failed`, e);
    }
  };

  tick();
  setInterval(tick, SAMPLE_INTERVAL_MS);
}

/** Mount the 3-day forecast. The demo provider is static so we don't
 *  set up a refresh interval - swap in a real provider and add one. */
function mountForecast(content: HTMLElement): void {
  content.innerHTML = `<section class="forecast"></section>`;
  const host = content.querySelector(".forecast") as HTMLElement;

  (async () => {
    try {
      const forecast = await getForecast();
      renderForecast(host, forecast);
    } catch (e) {
      console.error("[forecast] failed", e);
      host.textContent = "forecast unavailable";
    }
  })();
}

window.addEventListener("DOMContentLoaded", async () => {
  // Fetch the persisted lock state before we paint the chrome so the
  // icon and drag-region match on the very first frame.
  let initialLocked = false;
  try {
    initialLocked = await getWidgetLocked(widget);
  } catch (e) {
    console.error("[init] get_widget_locked failed", e);
  }

  const content = mountChrome(initialLocked);

  if (widget === "forecast") mountForecast(content);
  else mountSystemGauge(content, widget);
});
