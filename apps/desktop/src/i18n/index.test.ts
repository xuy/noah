// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";

function setNavigatorLanguage(value: string) {
  Object.defineProperty(window.navigator, "language", {
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

    expect(i18n.t("chat.submit")).toBe("Submit");
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
