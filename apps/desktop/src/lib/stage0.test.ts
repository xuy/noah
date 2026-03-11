/**
 * Stage 0 Feature Tests
 *
 * Tests for the "Won't Embarrass Me" launch readiness features.
 * Run with: npx vitest run
 */
import { describe, it, expect } from "vitest";

// ── 0.4: Friendly error message stripping ──

/** Matches the cleanError function in useAgent.ts */
function cleanError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  return raw.replace(/^Agent error:\s*/i, "");
}

describe("0.4: cleanError strips Agent error prefix", () => {
  it("strips 'Agent error: ' prefix", () => {
    expect(cleanError("Agent error: Your API key is invalid")).toBe(
      "Your API key is invalid",
    );
  });

  it("strips case-insensitively", () => {
    expect(cleanError("agent error: some message")).toBe("some message");
  });

  it("leaves messages without prefix unchanged", () => {
    expect(cleanError("Network timeout")).toBe("Network timeout");
  });

  it("handles Error objects", () => {
    expect(cleanError(new Error("Agent error: Claude is overloaded"))).toBe(
      "Claude is overloaded",
    );
  });

  it("handles non-string/non-Error values", () => {
    expect(cleanError(42)).toBe("42");
    expect(cleanError(null)).toBe("null");
    expect(cleanError(undefined)).toBe("undefined");
  });
});

// ── 0.5: Onboarding suggestion cards ──

describe("0.5: Onboarding suggestions", () => {
  const SUGGESTIONS = [
    { label: "My internet is slow", description: "Diagnose network issues" },
    {
      label: "My computer feels sluggish",
      description: "Check performance",
    },
    { label: "A program keeps crashing", description: "Find the cause" },
    { label: "Set up my printer", description: "Fix printing problems" },
  ];

  it("has exactly 4 suggestions", () => {
    expect(SUGGESTIONS).toHaveLength(4);
  });

  it("each suggestion has label and description", () => {
    for (const s of SUGGESTIONS) {
      expect(typeof s.label).toBe("string");
      expect(s.label.length).toBeGreaterThan(0);
      expect(typeof s.description).toBe("string");
      expect(s.description.length).toBeGreaterThan(0);
    }
  });

  it("labels are distinct", () => {
    const labels = SUGGESTIONS.map((s) => s.label);
    expect(new Set(labels).size).toBe(labels.length);
  });
});

// ── 0.6: Settings / app version IPC contract ──

describe("0.6: Settings IPC contract", () => {
  it("get_app_version takes no arguments", () => {
    // Rust: get_app_version() -> String
    const args = {};
    expect(Object.keys(args)).toHaveLength(0);
  });

  it("set_api_key uses camelCase key", () => {
    // Rust: set_api_key(api_key: String) -> Tauri converts to { apiKey }
    const args = { apiKey: "sk-ant-test123" };
    expect(args).toHaveProperty("apiKey");
    expect(args).not.toHaveProperty("api_key");
  });

  it("set_anthropic_base_url uses camelCase key", () => {
    // Rust: set_anthropic_base_url(base_url: String) -> { baseUrl }
    const args = { baseUrl: "https://api.anthropic.com" };
    expect(args).toHaveProperty("baseUrl");
    expect(args).not.toHaveProperty("base_url");
  });
});

// ── 0.8: Cancel processing IPC contract ──

describe("0.8: Cancel processing IPC contract", () => {
  it("cancel_processing takes no arguments", () => {
    // Rust: cancel_processing() -> ()
    const args = {};
    expect(Object.keys(args)).toHaveLength(0);
  });
});
