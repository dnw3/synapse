import { useCallback, useEffect, useState } from "react";

export type ThemeMode = "light" | "dark" | "system";

export interface AccentPreset {
  id: string;
  name: string;
  light: string;
  dark: string;
}

export const ACCENT_PRESETS: AccentPreset[] = [
  { id: "blue",     name: "Blue",     light: "#007AFF", dark: "#0A84FF" },
  { id: "purple",   name: "Purple",   light: "#A550A7", dark: "#BF5AF2" },
  { id: "pink",     name: "Pink",     light: "#F74F9E", dark: "#FF6482" },
  { id: "red",      name: "Red",      light: "#FF3B30", dark: "#FF453A" },
  { id: "orange",   name: "Orange",   light: "#FF9500", dark: "#FF9F0A" },
  { id: "yellow",   name: "Yellow",   light: "#FFCC00", dark: "#FFD60A" },
  { id: "green",    name: "Green",    light: "#34C759", dark: "#30D158" },
  { id: "graphite", name: "Graphite", light: "#8E8E93", dark: "#98989D" },
];

const STORAGE_KEY_MODE = "synapse-theme-mode";
const STORAGE_KEY_ACCENT = "synapse-accent";

function getSystemDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function resolveMode(mode: ThemeMode): "light" | "dark" {
  if (mode === "system") return getSystemDark() ? "dark" : "light";
  return mode;
}

/** Parse a hex color to HSL components */
function hexToHSL(hex: string): { h: number; s: number; l: number } {
  const r = parseInt(hex.slice(1, 3), 16) / 255;
  const g = parseInt(hex.slice(3, 5), 16) / 255;
  const b = parseInt(hex.slice(5, 7), 16) / 255;

  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;

  if (max === min) return { h: 0, s: 0, l };

  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h: number;
  if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
  else if (max === g) h = ((b - r) / d + 2) / 6;
  else h = ((r - g) / d + 4) / 6;

  return { h: h * 360, s, l };
}

function hslToHex(h: number, s: number, l: number): string {
  h = ((h % 360) + 360) % 360;
  s = Math.max(0, Math.min(1, s));
  l = Math.max(0, Math.min(1, l));

  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = l - c / 2;

  let r = 0, g = 0, b = 0;
  if (h < 60) { r = c; g = x; }
  else if (h < 120) { r = x; g = c; }
  else if (h < 180) { g = c; b = x; }
  else if (h < 240) { g = x; b = c; }
  else if (h < 300) { r = x; b = c; }
  else { r = c; b = x; }

  const toHex = (v: number) => Math.round((v + m) * 255).toString(16).padStart(2, "0");
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

function hexToRGB(hex: string): { r: number; g: number; b: number } {
  return {
    r: parseInt(hex.slice(1, 3), 16),
    g: parseInt(hex.slice(3, 5), 16),
    b: parseInt(hex.slice(5, 7), 16),
  };
}

/** Apply accent color and compute derived CSS vars via HSL manipulation */
function applyAccent(baseColor: string) {
  const el = document.documentElement;
  const hsl = hexToHSL(baseColor);
  const rgb = hexToRGB(baseColor);

  // --accent: base color directly
  el.style.setProperty("--accent", baseColor);

  // --accent-light: lighten by ~10%
  el.style.setProperty("--accent-light", hslToHex(hsl.h, hsl.s, Math.min(1, hsl.l + 0.1)));

  // --accent-glow: base @ 20% opacity
  el.style.setProperty("--accent-glow", `rgba(${rgb.r},${rgb.g},${rgb.b},0.20)`);

  // --accent-glow-strong: base @ 35% opacity
  el.style.setProperty("--accent-glow-strong", `rgba(${rgb.r},${rgb.g},${rgb.b},0.35)`);

  // --accent-gradient-end: shift hue +20deg, reduce saturation 10%
  el.style.setProperty(
    "--accent-gradient-end",
    hslToHex(hsl.h + 20, Math.max(0, hsl.s - 0.1), hsl.l)
  );
}

export function useTheme() {
  const [mode, setModeState] = useState<ThemeMode>(() => {
    const stored = localStorage.getItem(STORAGE_KEY_MODE);
    return (stored as ThemeMode) || "system";
  });

  const [accent, setAccentState] = useState<AccentPreset>(() => {
    const stored = localStorage.getItem(STORAGE_KEY_ACCENT);
    return ACCENT_PRESETS.find((p) => p.id === stored) || ACCENT_PRESETS[0];
  });

  const effectiveTheme = resolveMode(mode);

  // Apply theme mode via data-theme attribute
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", effectiveTheme);
  }, [effectiveTheme]);

  // Listen for system preference changes when mode is "system"
  useEffect(() => {
    if (mode !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      document.documentElement.setAttribute("data-theme", getSystemDark() ? "dark" : "light");
    };
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [mode]);

  // Apply accent — pick light/dark variant based on effective theme
  useEffect(() => {
    const baseColor = effectiveTheme === "light" ? accent.light : accent.dark;
    applyAccent(baseColor);
  }, [accent, effectiveTheme]);

  const setMode = useCallback((m: ThemeMode) => {
    setModeState(m);
    localStorage.setItem(STORAGE_KEY_MODE, m);
  }, []);

  const setAccent = useCallback((preset: AccentPreset) => {
    setAccentState(preset);
    localStorage.setItem(STORAGE_KEY_ACCENT, preset.id);
  }, []);

  const cycleMode = useCallback(() => {
    const order: ThemeMode[] = ["light", "dark", "system"];
    const next = order[(order.indexOf(mode) + 1) % order.length];
    setMode(next);
  }, [mode, setMode]);

  return {
    mode,
    setMode,
    effectiveTheme,
    /** @deprecated Use effectiveTheme instead */
    resolved: effectiveTheme,
    accent,
    setAccent,
    cycleMode,
    accentPresets: ACCENT_PRESETS,
  };
}
