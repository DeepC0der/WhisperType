import test from "node:test";
import assert from "node:assert/strict";
import {
  THEMES,
  resolveTheme,
  getTheme,
  isRetroTheme,
  applyThemeToDocument,
  type ThemeId,
} from "./theme.ts";

test("Theme Registry contains System and 7 distinct palettes", () => {
  const ids = THEMES.map((t) => t.id);
  const expected: ThemeId[] = [
    "system",
    "dark",
    "midnight",
    "amber",
    "matrix",
    "cyan",
    "light",
    "paper",
  ];

  assert.equal(THEMES.length, 8);
  for (const exp of expected) {
    assert.ok(ids.includes(exp), `Theme registry must include '${exp}'`);
  }

  for (const t of THEMES) {
    assert.ok(t.name.length > 0, `Theme '${t.id}' must have a name`);
    assert.ok(t.description.length > 0, `Theme '${t.id}' must have a description`);
    assert.ok(t.preview.background, `Theme '${t.id}' must have preview background`);
    assert.ok(t.preview.surface, `Theme '${t.id}' must have preview surface`);
    assert.ok(t.preview.primary, `Theme '${t.id}' must have preview primary`);
    assert.ok(t.preview.statusRecording, `Theme '${t.id}' must have preview statusRecording`);
  }
});

test("resolveTheme correctly resolves known themes", () => {
  assert.equal(resolveTheme("dark"), "dark");
  assert.equal(resolveTheme("amber"), "amber");
  assert.equal(resolveTheme("matrix"), "matrix");
  assert.equal(resolveTheme("cyan"), "cyan");
  assert.equal(resolveTheme("midnight"), "midnight");
  assert.equal(resolveTheme("light"), "light");
  assert.equal(resolveTheme("paper"), "paper");
});

test("resolveTheme safely falls back to 'dark' for unknown or legacy values", () => {
  assert.equal(resolveTheme(null), "dark");
  assert.equal(resolveTheme(undefined), "dark");
  assert.equal(resolveTheme(""), "dark");
  assert.equal(resolveTheme("unknown-theme-xyz"), "dark");
  assert.equal(resolveTheme("legacy-high-contrast"), "dark");
});

test("resolveTheme resolves 'system' according to matchMedia", () => {
  const originalWindow = globalThis.window;

  // Simulate dark OS preference
  (globalThis as unknown as { window: unknown }).window = {
    matchMedia: (query: string) => ({
      matches: query.includes("dark"),
      media: query,
      addEventListener: () => {},
      removeEventListener: () => {},
    }),
  };

  assert.equal(resolveTheme("system"), "dark");

  // Simulate light OS preference
  (globalThis as unknown as { window: unknown }).window = {
    matchMedia: () => ({
      matches: false,
      media: "",
      addEventListener: () => {},
      removeEventListener: () => {},
    }),
  };

  assert.equal(resolveTheme("system"), "light");

  // Restore
  (globalThis as unknown as { window: unknown }).window = originalWindow;
});

test("getTheme returns correct ThemeDefinition or falls back to 'dark'", () => {
  const midnight = getTheme("midnight");
  assert.equal(midnight.id, "midnight");
  assert.equal(midnight.mode, "dark");

  const paper = getTheme("paper");
  assert.equal(paper.id, "paper");
  assert.equal(paper.mode, "light");

  const fallback = getTheme("non-existent-theme");
  assert.equal(fallback.id, "dark");
});

test("applyThemeToDocument updates DOM root classes, color-scheme, and localStorage", () => {
  const classListSet = new Set<string>();
  const attributes = new Map<string, string>();
  const styleObj: Record<string, string> = {};
  const storage = new Map<string, string>();

  const mockRoot = {
    classList: {
      add: (...cls: string[]) => cls.forEach((c) => classListSet.add(c)),
      remove: (...cls: string[]) => cls.forEach((c) => classListSet.delete(c)),
      contains: (cls: string) => classListSet.has(cls),
    },
    style: styleObj,
    setAttribute: (name: string, val: string) => attributes.set(name, val),
    getAttribute: (name: string) => attributes.get(name),
  };

  const originalDoc = globalThis.document;
  const originalWindow = globalThis.window;

  (globalThis as unknown as { document: unknown }).document = {
    documentElement: mockRoot,
  };

  (globalThis as unknown as { window: unknown }).window = {
    localStorage: {
      setItem: (key: string, val: string) => storage.set(key, val),
      getItem: (key: string) => storage.get(key) || null,
    },
    matchMedia: (query: string) => ({
      matches: query.includes("dark"),
      media: query,
      addEventListener: () => {},
      removeEventListener: () => {},
    }),
  };

  try {
    // 1. Test Dark Theme (Midnight)
    const resMidnight = applyThemeToDocument("midnight");
    assert.equal(resMidnight, "midnight");
    assert.ok(mockRoot.classList.contains("dark"));
    assert.ok(mockRoot.classList.contains("theme-midnight"));
    assert.equal(mockRoot.style.colorScheme, "dark");
    assert.equal(attributes.get("data-theme"), "midnight");
    assert.equal(attributes.get("data-theme-setting"), "midnight");
    assert.equal(attributes.get("data-theme-style"), "matte");
    assert.equal(storage.get("murmur_theme"), "midnight");
    assert.equal(isRetroTheme("midnight"), false);

    // 2. Test Light Theme (Paper)
    const resPaper = applyThemeToDocument("paper");
    assert.equal(resPaper, "paper");
    assert.ok(!mockRoot.classList.contains("dark"));
    assert.ok(!mockRoot.classList.contains("theme-midnight"));
    assert.ok(mockRoot.classList.contains("theme-paper"));
    assert.equal(mockRoot.style.colorScheme, "light");
    assert.equal(attributes.get("data-theme"), "paper");
    assert.equal(attributes.get("data-theme-style"), "matte");
    assert.equal(storage.get("murmur_theme"), "paper");

    // 3. Test Optional Retro/Neon Theme (Amber)
    const resAmber = applyThemeToDocument("amber");
    assert.equal(resAmber, "amber");
    assert.equal(attributes.get("data-theme-style"), "retro");
    assert.equal(isRetroTheme("amber"), true);
    assert.equal(isRetroTheme("matrix"), true);
    assert.equal(isRetroTheme("cyan"), true);

    // 4. Test Unknown Theme Fallback
    const resFallback = applyThemeToDocument("bogus-theme");
    assert.equal(resFallback, "dark");
    assert.ok(mockRoot.classList.contains("dark"));
    assert.ok(!mockRoot.classList.contains("theme-paper"));
    assert.equal(mockRoot.style.colorScheme, "dark");
    assert.equal(attributes.get("data-theme"), "dark");
    assert.equal(attributes.get("data-theme-style"), "matte");
  } finally {
    (globalThis as unknown as { document: unknown }).document = originalDoc;
    (globalThis as unknown as { window: unknown }).window = originalWindow;
  }
});
