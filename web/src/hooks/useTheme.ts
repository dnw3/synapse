import { useCallback, useEffect, useState } from "react";

export type ThemeMode = "light" | "dark" | "system";

export interface AccentPreset {
  name: string;
  value: string;      // main accent
  light: string;      // lighter variant
  glow: string;       // glow rgba
  glowStrong: string; // stronger glow rgba
  gradient: string;   // logo gradient end
}

// MD3 Primary (Purple) preset for light theme
export const MD3_PRIMARY_PRESET: AccentPreset = {
  name: "purple", value: "#6750A4", light: "#7F67BE", glow: "rgba(103,80,164,0.15)", glowStrong: "rgba(103,80,164,0.25)", gradient: "#4F378B",
};

// Bitcoin Orange — default dark mode accent
export const BITCOIN_PRESET: AccentPreset = {
  name: "bitcoin", value: "#F7931A", light: "#FFD600", glow: "rgba(247,147,26,0.15)", glowStrong: "rgba(247,147,26,0.25)", gradient: "#EA580C",
};

export const ACCENT_PRESETS: AccentPreset[] = [
  BITCOIN_PRESET,
  { name: "blue",   value: "#2563eb", light: "#60a5fa", glow: "rgba(37,99,235,0.15)",  glowStrong: "rgba(37,99,235,0.25)",  gradient: "#1d4ed8" },
  { name: "teal",   value: "#0d9488", light: "#2dd4bf", glow: "rgba(13,148,136,0.15)", glowStrong: "rgba(13,148,136,0.25)", gradient: "#0f766e" },
  { name: "green",  value: "#16a34a", light: "#4ade80", glow: "rgba(22,163,74,0.15)",  glowStrong: "rgba(22,163,74,0.25)",  gradient: "#15803d" },
  { name: "orange", value: "#ea580c", light: "#fb923c", glow: "rgba(234,88,12,0.15)",  glowStrong: "rgba(234,88,12,0.25)",  gradient: "#c2410c" },
  { name: "rose",   value: "#e11d48", light: "#fb7185", glow: "rgba(225,29,72,0.15)",  glowStrong: "rgba(225,29,72,0.25)",  gradient: "#be123c" },
  { name: "violet", value: "#7c3aed", light: "#a78bfa", glow: "rgba(124,58,237,0.15)", glowStrong: "rgba(124,58,237,0.25)", gradient: "#6d28d9" },
  MD3_PRIMARY_PRESET,
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

function applyTheme(resolved: "light" | "dark") {
  document.documentElement.setAttribute("data-theme", resolved);
}

function applyAccent(preset: AccentPreset) {
  const el = document.documentElement;
  el.style.setProperty("--accent", preset.value);
  el.style.setProperty("--accent-light", preset.light);
  el.style.setProperty("--accent-glow", preset.glow);
  el.style.setProperty("--accent-glow-strong", preset.glowStrong);
  el.style.setProperty("--accent-gradient-end", preset.gradient);
}

export function useTheme() {
  const [mode, setModeState] = useState<ThemeMode>(() => {
    const stored = localStorage.getItem(STORAGE_KEY_MODE);
    return (stored as ThemeMode) || "system";
  });

  const [accent, setAccentState] = useState<AccentPreset>(() => {
    const stored = localStorage.getItem(STORAGE_KEY_ACCENT);
    return ACCENT_PRESETS.find((p) => p.name === stored) || ACCENT_PRESETS[0];
  });

  const resolved = resolveMode(mode);

  // Apply theme mode
  useEffect(() => {
    applyTheme(resolved);
  }, [resolved]);

  // Listen for system preference changes when mode is "system"
  useEffect(() => {
    if (mode !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => applyTheme(getSystemDark() ? "dark" : "light");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [mode]);

  // Apply accent — auto-switch to MD3 purple for light theme
  const prevResolvedRef = { current: resolved };
  useEffect(() => {
    if (resolved === "light") {
      applyAccent(MD3_PRIMARY_PRESET);
    } else {
      applyAccent(accent);
    }
    prevResolvedRef.current = resolved;
  }, [accent, resolved]);

  const setMode = useCallback((m: ThemeMode) => {
    setModeState(m);
    localStorage.setItem(STORAGE_KEY_MODE, m);
  }, []);

  const setAccent = useCallback((preset: AccentPreset) => {
    setAccentState(preset);
    localStorage.setItem(STORAGE_KEY_ACCENT, preset.name);
  }, []);

  const cycleMode = useCallback(() => {
    const order: ThemeMode[] = ["light", "dark", "system"];
    const next = order[(order.indexOf(mode) + 1) % order.length];
    setMode(next);
  }, [mode, setMode]);

  return { mode, resolved, accent, setMode, setAccent, cycleMode, presets: ACCENT_PRESETS };
}
