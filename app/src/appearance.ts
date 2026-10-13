// File > Settings > Appearance: theme and accent colour, applied as CSS
// variables on the root element (see the theme block in styles.css).

export type Theme = "dark" | "light" | "system";

export const DEFAULT_ACCENT = "#f8b133";

function hexToRgb(hex: string): [number, number, number] | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** Relative luminance (WCAG). */
function luminance([r, g, b]: [number, number, number]): number {
  const c = [r, g, b].map((v) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}

function scale([r, g, b]: [number, number, number], f: number): string {
  const c = (v: number) => Math.max(0, Math.min(255, Math.round(v * f)));
  return `rgb(${c(r)} ${c(g)} ${c(b)})`;
}

let media: MediaQueryList | null = null;
let current: { theme: Theme; accent: string } = { theme: "dark", accent: DEFAULT_ACCENT };

/** Apply a theme and accent colour now (and follow the OS for "system"). */
export function applyAppearance(theme: Theme, accent: string) {
  current = { theme, accent };
  const root = document.documentElement;
  const light = theme === "light" || (theme === "system" && window.matchMedia("(prefers-color-scheme: light)").matches);
  root.dataset.theme = light ? "light" : "dark";

  const rgb = hexToRgb(accent) ?? hexToRgb(DEFAULT_ACCENT)!;
  const lum = luminance(rgb);
  root.style.setProperty("--accent", `rgb(${rgb.join(" ")})`);
  root.style.setProperty("--accent-soft", `rgb(${rgb.join(" ")} / 0.16)`);
  // Text on an accent fill: black on light accents, white on dark ones.
  root.style.setProperty("--accent-ink", lum > 0.35 ? "#000000" : "#ffffff");
  // Accent-coloured text: on a light page, darken bright accents until readable.
  let text = `rgb(${rgb.join(" ")})`;
  if (light && lum > 0.2) {
    let f = 1;
    while (f > 0.3 && luminance(rgb.map((v) => v * f) as [number, number, number]) > 0.18) f -= 0.05;
    text = scale(rgb, f);
  } else if (!light && lum < 0.08) {
    text = scale(rgb, 1.8);
  }
  root.style.setProperty("--accent-text", text);

  if (theme === "system" && !media) {
    media = window.matchMedia("(prefers-color-scheme: light)");
    media.addEventListener("change", () => current.theme === "system" && applyAppearance(current.theme, current.accent));
  }
}
