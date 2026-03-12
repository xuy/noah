import { useState, useEffect, useCallback } from "react";
import en from "./en.json";
import es from "./es.json";
import zh from "./zh.json";

export type Locale = "en" | "es" | "zh";
export type LocalePreference = "auto" | Locale;
export type LocaleOption = {
  value: LocalePreference;
  labelKey: string;
  descKey: string;
};

const STORAGE_KEY = "noah-locale";

const messages: Record<Locale, Record<string, unknown>> = { en, es, zh };
export const LOCALE_OPTIONS: LocaleOption[] = [
  { value: "auto", labelKey: "settings.auto", descKey: "settings.langAutoDesc" },
  { value: "en", labelKey: "settings.langEnglish", descKey: "settings.langEnDesc" },
  { value: "es", labelKey: "settings.langSpanish", descKey: "settings.langEsDesc" },
  { value: "zh", labelKey: "settings.langChinese", descKey: "settings.langZhDesc" },
];

function getStored(): LocalePreference {
  const v = localStorage.getItem(STORAGE_KEY);
  if (v === "en" || v === "es" || v === "zh") return v;
  return "auto";
}

function detectLocale(): Locale {
  const lang = navigator.language.toLowerCase();
  if (lang.startsWith("es")) return "es";
  if (lang.startsWith("zh")) return "zh";
  return "en";
}

function resolveLocale(pref: LocalePreference): Locale {
  if (pref === "auto") return detectLocale();
  return pref;
}

/** Look up a nested key like "chat.placeholder" in a messages object. */
function lookup(obj: Record<string, unknown>, key: string): unknown {
  const parts = key.split(".");
  let current: unknown = obj;
  for (const part of parts) {
    if (current == null || typeof current !== "object") return undefined;
    current = (current as Record<string, unknown>)[part];
  }
  return current;
}

/**
 * Translate a key with optional interpolation.
 * `t("chat.stepOf", { step: 1, total: 5 })` → "Step 1 of 5"
 *
 * For pluralisation-like patterns, use `{s}` which resolves to "" if count === 1, "s" otherwise.
 * Pass `count` in the params to enable this.
 */
function translate(
  locale: Locale,
  key: string,
  params?: Record<string, string | number>,
): string {
  let value = lookup(messages[locale], key);
  // Fall back to English if not found in current locale.
  if (value === undefined) {
    value = lookup(messages.en, key);
  }
  if (value === undefined) return key;
  if (typeof value !== "string") return key;

  let result = value;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      result = result.replace(new RegExp(`\\{${k}\\}`, "g"), String(v));
    }
    // Handle {s} plural suffix: "" when count === 1, "s" otherwise.
    if ("count" in params) {
      result = result.replace(/\{s\}/g, params.count === 1 ? "" : "s");
    }
  }
  return result;
}

/**
 * Look up an array value (e.g. "setup.taglines").
 */
function translateArray(locale: Locale, key: string): string[] {
  let value = lookup(messages[locale], key);
  if (!Array.isArray(value)) {
    value = lookup(messages.en, key);
  }
  if (!Array.isArray(value)) return [];
  return value as string[];
}

// Singleton state so all hook instances share the same locale.
let _locale: Locale = resolveLocale(getStored());
let _preference: LocalePreference = getStored();
const _listeners = new Set<() => void>();

function setGlobalLocale(pref: LocalePreference) {
  _preference = pref;
  _locale = resolveLocale(pref);
  localStorage.setItem(STORAGE_KEY, pref);
  for (const fn of _listeners) fn();
}

export function useLocale() {
  const [, forceUpdate] = useState(0);

  useEffect(() => {
    const listener = () => forceUpdate((n) => n + 1);
    _listeners.add(listener);
    return () => { _listeners.delete(listener); };
  }, []);

  const t = useCallback(
    (key: string, params?: Record<string, string | number>) =>
      translate(_locale, key, params),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [_locale],
  );

  const tArray = useCallback(
    (key: string) => translateArray(_locale, key),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [_locale],
  );

  const setLocale = useCallback((pref: LocalePreference) => {
    setGlobalLocale(pref);
  }, []);

  return {
    locale: _locale,
    preference: _preference,
    t,
    tArray,
    setLocale,
  };
}

/** Non-hook version for use outside React components (e.g. constants). */
export function t(key: string, params?: Record<string, string | number>): string {
  return translate(_locale, key, params);
}

export function currentLocale(): Locale {
  return _locale;
}
