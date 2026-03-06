import { useState, useEffect, useCallback } from "react";

export type ThemePreference = "system" | "light" | "dark";

const STORAGE_KEY = "noah-theme";

function getStored(): ThemePreference {
  const v = localStorage.getItem(STORAGE_KEY);
  if (v === "light" || v === "dark") return v;
  return "system";
}

function applyTheme(pref: ThemePreference) {
  const isLight =
    pref === "light" ||
    (pref === "system" &&
      window.matchMedia("(prefers-color-scheme: light)").matches);

  document.documentElement.classList.toggle("light", isLight);
}

export function useTheme() {
  const [preference, setPreference] = useState<ThemePreference>(getStored);

  // Apply on mount and when preference changes.
  useEffect(() => {
    applyTheme(preference);
    localStorage.setItem(STORAGE_KEY, preference);
  }, [preference]);

  // When "system" is selected, track OS changes.
  useEffect(() => {
    if (preference !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const handler = () => applyTheme("system");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [preference]);

  const setTheme = useCallback((pref: ThemePreference) => {
    setPreference(pref);
  }, []);

  return { preference, setTheme };
}
