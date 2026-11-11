/**
 * Complete typed theme registry and management for WhisperType.
 *
 * Supported Themes:
 * Matte Finish Palettes (Default):
 * - "system": Follows Windows OS light/dark preference with live change detection
 * - "dark": Matte Obsidian - Sleek matte charcoal workspace matching the website (Default)
 * - "midnight": Matte Slate - Restrained deep navy/slate low-distraction matte workspace
 * - "light": Matte Alabaster - Crisp modern matte light workspace
 * - "paper": Matte Parchment - Warm, bookish matte paper surface
 *
 * Retro & Neon CRT Palettes (Optional):
 * - "amber": Retro Amber CRT - Warm CRT monochrome amber phosphor with pixel typography
 * - "matrix": Matrix Neon - High-contrast terminal neon green with CRT scanlines
 * - "cyan": Cyberpunk Cyan - Electric cyan & magenta neon with retro pixel styling
 */

export type ThemeMode = "dark" | "light";
export type ThemeCategory = "matte" | "retro";

export type ThemeId =
  | "system"
  | "dark"
  | "amber"
  | "matrix"
  | "cyan"
  | "midnight"
  | "light"
  | "paper";

export type ResolvedThemeId =
  | "dark"
  | "amber"
  | "matrix"
  | "cyan"
  | "midnight"
  | "light"
  | "paper";

export interface ThemeDefinition {
  id: ThemeId;
  name: string;
  mode: ThemeMode | "auto";
  category: ThemeCategory;
  description: string;
  preview: {
    background: string;
    surface: string;
    border: string;
    primary: string;
    foreground: string;
    statusRecording: string;
    accent?: string;
  };
}

export const THEMES: ThemeDefinition[] = [
  {
    id: "system",
    name: "System Matte",
    mode: "auto",
    category: "matte",
    description: "Follows Windows OS light/dark preference with a clean matte finish.",
    preview: {
      background: "#0c0f13",
      surface: "#13171f",
      border: "#222c3d",
      primary: "#35e28a",
      foreground: "#f3f6fa",
      statusRecording: "#ef4444",
      accent: "#f5f7fa",
    },
  },
  {
    id: "dark",
    name: "Matte Obsidian",
    mode: "dark",
    category: "matte",
    description: "Signature website matte charcoal surface with understated emerald accents.",
    preview: {
      background: "#0c0f13",
      surface: "#13171f",
      border: "#222c3d",
      primary: "#35e28a",
      foreground: "#f3f6fa",
      statusRecording: "#ef4444",
    },
  },
  {
    id: "midnight",
    name: "Matte Slate",
    mode: "dark",
    category: "matte",
    description: "Restrained deep slate & indigo matte workspace for low-glare focus.",
    preview: {
      background: "#0a0d14",
      surface: "#111622",
      border: "#222d42",
      primary: "#5a8afc",
      foreground: "#f0f3fa",
      statusRecording: "#ed4e6b",
    },
  },
  {
    id: "light",
    name: "Matte Alabaster",
    mode: "light",
    category: "matte",
    description: "Crisp modern matte light workspace with soft borders and deep emerald accents.",
    preview: {
      background: "#f5f7fa",
      surface: "#ffffff",
      border: "#d0d7e2",
      primary: "#18a85f",
      foreground: "#171d26",
      statusRecording: "#dc2626",
    },
  },
  {
    id: "paper",
    name: "Matte Parchment",
    mode: "light",
    category: "matte",
    description: "Warm, glare-free matte paper surface with high-contrast espresso ink.",
    preview: {
      background: "#f7f4ed",
      surface: "#faf8f3",
      border: "#d8cfc0",
      primary: "#c4581c",
      foreground: "#2e241c",
      statusRecording: "#d52d24",
    },
  },
  {
    id: "amber",
    name: "Retro Amber CRT",
    mode: "dark",
    category: "retro",
    description: "Optional retro-modern CRT amber phosphor with pixel typography and scanlines.",
    preview: {
      background: "#140d06",
      surface: "#1c140a",
      border: "#3b2816",
      primary: "#faa018",
      foreground: "#fef7ec",
      statusRecording: "#f14d38",
    },
  },
  {
    id: "matrix",
    name: "Matrix Neon",
    mode: "dark",
    category: "retro",
    description: "Optional retro-modern phosphor black with high-contrast neon green glow.",
    preview: {
      background: "#040805",
      surface: "#09120a",
      border: "#14351a",
      primary: "#00ff40",
      foreground: "#eaffed",
      statusRecording: "#f04343",
    },
  },
  {
    id: "cyan",
    name: "Cyberpunk Neon",
    mode: "dark",
    category: "retro",
    description: "Optional retro-modern electric cyan & magenta neon on deep ocean black.",
    preview: {
      background: "#060e12",
      surface: "#0b171c",
      border: "#183844",
      primary: "#12e0f7",
      foreground: "#e8fbfe",
      statusRecording: "#f74577",
    },
  },
];

const KNOWN_RESOLVED_IDS: Set<ResolvedThemeId> = new Set([
  "dark",
  "amber",
  "matrix",
  "cyan",
  "midnight",
  "light",
  "paper",
]);

const RETRO_THEME_IDS: Set<ResolvedThemeId> = new Set([
  "amber",
  "matrix",
  "cyan",
]);

let mediaQueryListener: ((e: MediaQueryListEvent) => void) | null = null;
let currentMediaQuery: MediaQueryList | null = null;

/**
 * Returns true if the resolved theme uses the optional retro-modern / neon CRT aesthetic.
 */
export function isRetroTheme(themeId?: string | null): boolean {
  const resolved = resolveTheme(themeId);
  return RETRO_THEME_IDS.has(resolved);
}

/**
 * Resolves any theme ID or legacy setting string to a concrete ResolvedThemeId.
 * - "system": evaluates OS prefers-color-scheme (dark -> "dark", light -> "light")
 * - Known IDs: returned as-is
 * - Unknown / empty / invalid: safely falls back to "dark"
 */
export function resolveTheme(themeId?: string | null): ResolvedThemeId {
  if (!themeId) return "dark";

  if (themeId === "system") {
    if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
      const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      return isDark ? "dark" : "light";
    }
    return "dark";
  }

  if (KNOWN_RESOLVED_IDS.has(themeId as ResolvedThemeId)) {
    return themeId as ResolvedThemeId;
  }

  // Safe fallback for unknown or legacy values
  return "dark";
}

/**
 * Returns the theme definition for an ID, safely falling back to "dark".
 */
export function getTheme(themeId?: string | null): ThemeDefinition {
  const match = THEMES.find((t) => t.id === themeId);
  if (match) return match;
  return THEMES.find((t) => t.id === "dark")!;
}

/**
 * Applies the theme classes, color-scheme, and data attributes to document.documentElement.
 * Also synchronizes localStorage so new windows load without flash.
 * If "system" is chosen, attaches a live listener for OS preference changes.
 */
export function applyThemeToDocument(themeId?: string | null): ResolvedThemeId {
  const effectiveId = themeId || "dark";
  const resolved = resolveTheme(effectiveId);
  const retro = RETRO_THEME_IDS.has(resolved);

  // Manage live OS preference listener
  if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
    if (effectiveId === "system") {
      if (!mediaQueryListener) {
        currentMediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
        mediaQueryListener = () => {
          applyThemeToDocument("system");
        };
        currentMediaQuery.addEventListener("change", mediaQueryListener);
      }
    } else if (mediaQueryListener && currentMediaQuery) {
      currentMediaQuery.removeEventListener("change", mediaQueryListener);
      mediaQueryListener = null;
      currentMediaQuery = null;
    }
  }

  // Persist to localStorage for early inline script consumption in index.html
  if (typeof window !== "undefined" && window.localStorage) {
    try {
      window.localStorage.setItem("murmur_theme", effectiveId);
      window.localStorage.setItem("murmur_resolved_theme", resolved);
    } catch {
      // Ignore quota or permission errors
    }
  }

  if (typeof document === "undefined") {
    return resolved;
  }

  const root = document.documentElement;

  // Clear all theme markers
  root.classList.remove(
    "dark",
    "theme-dark",
    "theme-midnight",
    "theme-amber",
    "theme-matrix",
    "theme-cyan",
    "theme-light",
    "theme-paper"
  );

  const isLight = resolved === "light" || resolved === "paper";

  if (isLight) {
    root.classList.add(`theme-${resolved}`);
    root.style.colorScheme = "light";
  } else {
    root.classList.add("dark");
    if (resolved !== "dark") {
      root.classList.add(`theme-${resolved}`);
    }
    root.style.colorScheme = "dark";
  }

  if (typeof document.querySelector === "function") {
    const meta = document.querySelector('meta[name="color-scheme"]');
    if (meta) {
      meta.setAttribute("content", effectiveId === "system" ? "light dark" : isLight ? "light" : "dark");
    }
  }

  root.setAttribute("data-theme", resolved);
  root.setAttribute("data-theme-setting", effectiveId);
  root.setAttribute("data-theme-style", retro ? "retro" : "matte");

  return resolved;
}
