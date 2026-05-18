// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

function setNavigatorLanguage(value: string) {
  Object.defineProperty(window.navigator, "language", {
    configurable: true,
    value,
  });
}

function setNavigatorPlatform(value: string) {
  Object.defineProperty(window.navigator, "platform", {
    configurable: true,
    value,
  });
}

async function loadI18n() {
  vi.resetModules();
  return import("./index");
}

describe("i18n locale resolution", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("uses Spanish when OS language is Spanish and preference is auto", async () => {
    setNavigatorLanguage("es-ES");
    const i18n = await loadI18n();

    expect(i18n.currentLocale()).toBe("es");
    expect(i18n.t("settings.title")).toBe("Configuración");
  });

  it("respects stored Spanish preference", async () => {
    setNavigatorLanguage("en-US");
    localStorage.setItem("noah-locale", "es");
    const i18n = await loadI18n();

    expect(i18n.currentLocale()).toBe("es");
  });

  it("falls back to English when a Spanish key is not defined", async () => {
    setNavigatorLanguage("es-MX");
    const i18n = await loadI18n();

    // Pick a key that's English-only — `signIn.welcomeTitle` is not in
    // es.json today, so the fallback path is exercised. Update this
    // assertion if/when the key gets translated.
    expect(i18n.t("signIn.welcomeTitle")).toBe("Sign in to Noah");
  });

  it("exposes Spanish as a selectable language option", async () => {
    const i18n = await loadI18n();

    expect(i18n.LOCALE_OPTIONS.map((opt) => opt.value)).toEqual([
      "auto",
      "en",
      "es",
      "zh",
    ]);
  });
});

describe("i18n platform token substitution", () => {
  beforeEach(() => {
    // Tests in the prior describe block leave navigator.language as
    // "es-MX" or similar; force English here so we're testing the
    // {device} substitution against known en.json strings rather than
    // whatever locale happens to load.
    localStorage.clear();
    setNavigatorLanguage("en-US");
  });

  afterEach(() => {
    // Restore the test-setup default so other test files aren't surprised.
    setNavigatorPlatform("MacIntel");
  });

  it("substitutes {device} as 'Mac' on macOS", async () => {
    setNavigatorPlatform("MacIntel");
    const i18n = await loadI18n();
    expect(i18n.t("onboarding.greeting")).toBe(
      "Hi, I'm Noah. What's going on with your Mac?",
    );
    expect(i18n.t("onboarding.tile.slow.title")).toBe("My Mac feels slow");
  });

  it("substitutes {device} as 'PC' and {osName} as 'Windows' on Windows", async () => {
    setNavigatorPlatform("Win32");
    const i18n = await loadI18n();
    expect(i18n.t("onboarding.greeting")).toBe(
      "Hi, I'm Noah. What's going on with your PC?",
    );
    expect(i18n.t("onboarding.tile.slow.title")).toBe("My PC feels slow");
    expect(i18n.t("onboarding.tile.update.desc")).toBe(
      "A new bug after Windows or an app updated",
    );
  });
});
