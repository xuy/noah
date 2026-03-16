import { create } from "zustand";
import type { HealthScore } from "../lib/tauri-commands";

interface HealthState {
  score: HealthScore | null;
  history: HealthScore[];
  loading: boolean;
  error: string | null;

  setScore: (score: HealthScore) => void;
  setHistory: (history: HealthScore[]) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
}

export const useHealthStore = create<HealthState>((set) => ({
  score: null,
  history: [],
  loading: false,
  error: null,

  setScore: (score) => set({ score, error: null }),
  setHistory: (history) => set({ history }),
  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error, loading: false }),
}));
