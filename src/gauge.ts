// SVG-based animated ring gauge.
//
// Visual structure of every gauge:
//
//     ┌───────────────┐
//     │     ╭───╮      │   <- faint full ring  (the "track")
//     │   ╱       ╲    │   <- coloured arc     (grows clockwise)
//     │  │   42%   │   │   <- big % label      (centre)
//     │   ╲ 6/16GB ╱   │   <- subtitle         (just below)
//     │     ╰───╯      │
//     └───────────────┘
//
// Why SVG and not <canvas>? We tried both: a plain 2D context, then
// an OffscreenCanvas + ImageBitmapRenderingContext atomic-swap
// pipeline. Both produced a "ghost / stack" artefact on Tauri's
// WebKitGTK WebView, where the transparent compositor surface
// retained previous-frame pixels: the faint track creeped toward
// solid white, the colored arc smeared, and percentage labels piled
// up on top of each other until something happened to invalidate the
// layer. The bug lives in the compositor, below the rendering API -
// even `clearRect`, `canvas.width` reassignment, and bitmap-
// replacement transfers did not propagate fully to the cached layer.
//
// SVG is preferable here because vector primitives are declarative:
// updates are minimal attribute / textContent edits rather than a
// full per-frame redraw, which keeps the rendering work small and
// the DOM the single source of truth. But it does NOT make the
// compositor bug go away by itself - the same ghost / stack
// reproduces with SVG, since both pipelines ultimately paint into
// the same transparent WebView surface whose cached layer fails to
// invalidate cleanly between repaints.
//
// What actually defeats it: an opaque (or near-opaque) layer
// inside the gauge's paint stack. We add a "puck" - a filled
// circle drawn behind the ring and the text labels - using a
// semi-opaque dark colour that matches the day-card backdrop in
// style.css. With that puck present, every pixel WebKit needs to
// repaint sits on top of an opaque destination, so the
// surface-clear step that was previously a no-op now actually
// erases the previous frame.
//
// Two earlier in-file attempts did NOT work and have been removed:
//
//   1. Flipping a no-op `transform: translate3d` on the <svg> with
//      `will-change: transform`. A transform change just moves the
//      cached layer's bitmap to a new position - the compositor is
//      explicitly designed to do that without re-rasterising the
//      contents.
//   2. Detaching and re-inserting the <svg> into its parent on
//      every render(). Forces a fresh layer, but the destination
//      surface is still transparent, so the freshly-painted bitmap
//      composites *over* the previous frame's pixels instead of
//      replacing them.
//
// Neither helped because this is not a cache-coherency bug. It is
// a "clear-to-what" bug: when WebKit repaints a transparent layer
// it has no canonical colour to erase the destination to, so the
// previous frame's pixels are left in place and the new pixels are
// alpha-composited on top. No amount of DOM invalidation reaches
// that surface-clear step. The puck supplies the missing
// destination colour without touching the surrounding window
// chrome (the SVG corners outside the puck stay transparent, so
// the widget still looks like a round gauge floating over the
// desktop).
//
// Animation strategy is unchanged: each `setValue()` call only
// stores a target, and a requestAnimationFrame tween moves the
// displayed value toward it with an ease-out curve. The loop
// self-parks once the gap closes and re-arms only when the next
// distinct sample arrives.

const SVG_NS = "http://www.w3.org/2000/svg";

// All geometry is expressed in viewBox units (0..100). CSS scales
// the SVG to whatever pixel size the container actually has, so we
// never need to think about devicePixelRatio or HiDPI here.
const RADIUS = 42;
const STROKE_WIDTH = 8;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

export interface GaugeOptions {
  /** Arc colour while value is below `warnAt`. Default: blue. */
  color?: string;
  /** Arc colour once value crosses `warnAt`. Default: amber. */
  warnColor?: string;
  /** Arc colour once value crosses `dangerAt`. Default: red. */
  dangerColor?: string;
  /** Threshold for `warnColor`. Default 70. */
  warnAt?: number;
  /** Threshold for `dangerColor`. Default 90. */
  dangerAt?: number;
  /** Subtitle drawn under the percentage. */
  subtitle?: string;
}

type RequiredGaugeOptions = Required<
  Pick<
    GaugeOptions,
    "color" | "warnColor" | "dangerColor" | "warnAt" | "dangerAt"
  >
>;

export class Gauge {
  private readonly svg: SVGSVGElement;
  private readonly arc: SVGCircleElement;
  private readonly valueText: SVGTextElement;
  private readonly subtitleText: SVGTextElement;
  private displayedValue = 0;
  private targetValue = 0;
  private rafHandle: number | null = null;
  private subtitle: string;
  private disabled = false;
  private readonly opts: RequiredGaugeOptions;

  /** Build the gauge inside the given container element. The
   *  container is expected to be a block-level element (e.g. a
   *  `<div>`); the gauge stretches to fill its width/height via CSS
   *  on the inner `<svg>`. */
  constructor(container: HTMLElement, options: GaugeOptions = {}) {
    this.opts = {
      color: options.color ?? "#5fb3ff",
      warnColor: options.warnColor ?? "#ffb55f",
      dangerColor: options.dangerColor ?? "#ff6b6b",
      warnAt: options.warnAt ?? 70,
      dangerAt: options.dangerAt ?? 90,
    };
    this.subtitle = options.subtitle ?? "";

    this.svg = document.createElementNS(SVG_NS, "svg") as SVGSVGElement;
    this.svg.setAttribute("viewBox", "0 0 100 100");
    this.svg.setAttribute("class", "gauge-svg");

    // Backdrop puck: an opaque-enough circle behind the ring and
    // text. Its only job is to give WebKitGTK an opaque destination
    // to clear to before each repaint - see the top-of-file comment
    // for the bug it fixes. Radius 48 covers the ring (r 38..46
    // including stroke width) plus the centred glyphs with a tiny
    // halo of margin, and stops short of the viewBox edge so the
    // SVG corners stay transparent and the widget reads as a round
    // gauge floating on the desktop. Colour matches the day-card
    // backdrop already in style.css for visual consistency.
    const puck = document.createElementNS(SVG_NS, "circle");
    puck.setAttribute("cx", "50");
    puck.setAttribute("cy", "50");
    puck.setAttribute("r", "48");
    puck.setAttribute("fill", "rgb(0, 0, 0)");
    this.svg.appendChild(puck);

    // Track: a faint full-circle stroke that doesn't move.
    const track = document.createElementNS(SVG_NS, "circle");
    track.setAttribute("cx", "50");
    track.setAttribute("cy", "50");
    track.setAttribute("r", String(RADIUS));
    track.setAttribute("fill", "none");
    track.setAttribute("stroke", "rgba(255, 255, 255, 0.01)");
    track.setAttribute("stroke-width", String(STROKE_WIDTH));
    this.svg.appendChild(track);

    // Active arc: same circle, but only a slice is visible.
    //
    // Trick: stroke-dasharray = "L C" draws a dash of length L
    // followed by a gap of the full circumference C, so we get a
    // single arc of length L. Rotating the circle -90deg around its
    // centre puts the start of that dash at the top (12 o'clock)
    // instead of the right (3 o'clock).
    this.arc = document.createElementNS(SVG_NS, "circle") as SVGCircleElement;
    this.arc.setAttribute("cx", "50");
    this.arc.setAttribute("cy", "50");
    this.arc.setAttribute("r", String(RADIUS));
    this.arc.setAttribute("fill", "none");
    this.arc.setAttribute("stroke", this.opts.color);
    this.arc.setAttribute("stroke-width", String(STROKE_WIDTH));
    this.arc.setAttribute("stroke-linecap", "round");
    this.arc.setAttribute("stroke-dasharray", `0 ${CIRCUMFERENCE}`);
    this.arc.setAttribute("transform", "rotate(-90 50 50)");
    this.svg.appendChild(this.arc);

    // Centre value label.
    this.valueText = document.createElementNS(SVG_NS, "text") as SVGTextElement;
    this.valueText.setAttribute("x", "50");
    this.valueText.setAttribute("text-anchor", "middle");
    this.valueText.setAttribute("dominant-baseline", "central");
    this.valueText.setAttribute("class", "gauge-value");
    this.svg.appendChild(this.valueText);

    // Subtitle under the value.
    this.subtitleText = document.createElementNS(SVG_NS, "text") as SVGTextElement;
    this.subtitleText.setAttribute("x", "50");
    this.subtitleText.setAttribute("y", "64");
    this.subtitleText.setAttribute("text-anchor", "middle");
    this.subtitleText.setAttribute("dominant-baseline", "central");
    this.subtitleText.setAttribute("class", "gauge-subtitle");
    this.svg.appendChild(this.subtitleText);

    container.appendChild(this.svg);

    this.render();
  }

  /** Update the subtitle (e.g. "12 / 32 GB") and rerender. */
  setSubtitle(text: string): void {
    if (text === this.subtitle) return;
    this.subtitle = text;
    this.render();
  }

  /** Toggle the "no source available" state - shows N/A. */
  setDisabled(state: boolean): void {
    if (state === this.disabled) return;
    this.disabled = state;
    this.render();
  }

  /** Set the target value (0..100); the animation loop will tween. */
  setValue(v: number): void {
    const clamped = clamp(v, 0, 100);
    // If we're idle and already showing this exact value, there's
    // nothing to animate toward and nothing to repaint. Skipping here
    // avoids redundant DOM writes on every 1-Hz sample whenever the
    // metric is steady (which is most of the time for CPU/RAM/VRAM).
    if (clamped === this.targetValue && this.rafHandle == null) return;
    this.targetValue = clamped;
    // Kick the rAF loop only if it isn't already running. tick() will
    // self-park once it reaches the target.
    if (this.rafHandle == null) this.tick();
  }

  /** Blank the readout - arc, value, and subtitle all empty. Useful
   *  when a widget wants to hide its data without unmounting. */
  clear(): void {
    this.arc.setAttribute("stroke-dasharray", `0 ${CIRCUMFERENCE}`);
    this.valueText.textContent = "";
    this.subtitleText.textContent = "";
  }

  // ---------------------------------------------------------------
  // internals
  // ---------------------------------------------------------------

  // Arrow form so `this` is bound when passed to requestAnimationFrame.
  private tick = (): void => {
    // Ease-out: each frame we close 12% of the remaining gap. At
    // ~60 fps this reaches the target in ~250-400 ms, which feels
    // snappy on a 1-Hz update cadence without overshooting.
    const delta = this.targetValue - this.displayedValue;
    if (Math.abs(delta) < 0.05) {
      this.displayedValue = this.targetValue;
      this.rafHandle = null;
    } else {
      this.displayedValue += delta * 0.12;
      this.rafHandle = requestAnimationFrame(this.tick);
    }
    this.render();
  };

  private currentColor(): string {
    if (this.displayedValue >= this.opts.dangerAt) return this.opts.dangerColor;
    if (this.displayedValue >= this.opts.warnAt) return this.opts.warnColor;
    return this.opts.color;
  }

  /** Push the current state to the SVG DOM. Plain attribute /
   *  textContent writes - no compositor tricks. See the top-of-file
   *  comment for why DOM-level cache busting could not solve the
   *  WebKitGTK ghosting bug and where the real fix lives. */
  private render(): void {
    if (this.disabled) {
      this.arc.setAttribute("stroke-dasharray", `0 ${CIRCUMFERENCE}`);
      this.valueText.textContent = "N/A";
    } else {
      const len = (CIRCUMFERENCE * this.displayedValue) / 100;
      this.arc.setAttribute("stroke-dasharray", `${len} ${CIRCUMFERENCE}`);
      this.arc.setAttribute("stroke", this.currentColor());
      this.valueText.textContent = `${Math.round(this.displayedValue)}%`;
    }
    // Shift the value up to make room for the subtitle when there is
    // one - mirrors the y-offset trick from the old canvas renderer.
    this.valueText.setAttribute("y", this.subtitle ? "44" : "50");
    this.subtitleText.textContent = this.subtitle;
  }
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}
