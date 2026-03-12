// Vitest setup file
// @testing-library/jest-dom matchers are not used; standard vitest
// assertions + getByText/queryByText handle all test assertions.

// Node 25 exposes a stub localStorage global that shadows jsdom's full
// implementation (missing getItem, setItem, clear, etc.). Patch it so
// browser APIs work in both node and jsdom environments.
if (typeof globalThis.localStorage?.getItem !== "function") {
  const store = new Map<string, string>();
  globalThis.localStorage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => { store.set(k, String(v)); },
    removeItem: (k: string) => { store.delete(k); },
    clear: () => { store.clear(); },
    get length() { return store.size; },
    key: (i: number) => [...store.keys()][i] ?? null,
  } as Storage;
}
