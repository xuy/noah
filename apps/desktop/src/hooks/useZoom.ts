import { useEffect, useState } from "react";

const STEP = 0.1;
const MIN = 0.5;
const MAX = 2.0;
const STORAGE_KEY = "noah-zoom";

function loadZoom(): number {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) return Math.max(MIN, Math.min(MAX, parseFloat(stored)));
  } catch {}
  return 1;
}

/**
 * Content-only zoom controlled by Cmd+/-/0.
 * Returns the zoom value — the caller applies it to the content container,
 * leaving the title bar and sidebar unzoomed (and aligned with traffic lights).
 */
export function useZoom() {
  const [zoom, setZoom] = useState(loadZoom);

  // Persist zoom level
  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, String(zoom));
    } catch {}
  }, [zoom]);

  // Listen for Cmd+/-/0
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;

      if (e.key === "=" || e.key === "+") {
        e.preventDefault();
        setZoom((z) => Math.min(z + STEP, MAX));
      } else if (e.key === "-") {
        e.preventDefault();
        setZoom((z) => Math.max(z - STEP, MIN));
      } else if (e.key === "0") {
        e.preventDefault();
        setZoom(1);
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return zoom;
}
