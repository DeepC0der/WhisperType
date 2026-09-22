/**
 * Theme management for Murmur.
 * Standardizes theme class application across all windows:
 * - "dark" (Phosphor Green): dark (no theme-* classes)
 * - "amber": dark theme-amber
 * - "matrix": dark theme-matrix
 * - "cyan": dark theme-cyan
 * - "light": theme-light (no dark)
 */
export function applyThemeToDocument(theme: string): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;

  // Clear existing theme markers
  root.classList.remove("dark", "theme-light", "theme-amber", "theme-matrix", "theme-cyan");

  switch (theme) {
    case "light":
      root.classList.add("theme-light");
      break;
    case "amber":
      root.classList.add("dark", "theme-amber");
      break;
    case "matrix":
      root.classList.add("dark", "theme-matrix");
      break;
    case "cyan":
      root.classList.add("dark", "theme-cyan");
      break;
    case "dark":
    default:
      root.classList.add("dark");
      break;
  }
}
