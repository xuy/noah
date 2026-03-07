import { describe, it, expect } from "vitest";
import { parseResponse } from "./parseResponse";

describe("parseResponse", () => {
  // ── Action cards ──

  it("parses a full action response", () => {
    const raw = `[SITUATION]
Your iPhone "Alex's iPhone" is available as a Wi-Fi hotspot nearby.
[PLAN]
Connect this Mac to your iPhone's hotspot via Wi-Fi.
[ACTION:Connect]`;

    const result = parseResponse(raw);
    expect(result).toEqual({
      type: "action",
      situation: `Your iPhone "Alex's iPhone" is available as a Wi-Fi hotspot nearby.`,
      plan: "Connect this Mac to your iPhone's hotspot via Wi-Fi.",
      actionLabel: "Connect",
    });
  });

  it("parses action with multi-word button label", () => {
    const raw = `[SITUATION]
DNS cache is stale.
[PLAN]
Flush the DNS cache to resolve the lookup failures.
[ACTION:Fix it]`;

    const result = parseResponse(raw);
    expect(result.type).toBe("action");
    if (result.type === "action") {
      expect(result.actionLabel).toBe("Fix it");
    }
  });

  it("handles extra whitespace around sections", () => {
    const raw = `
  [SITUATION]

  Your printer queue is stuck with 3 jobs.

  [PLAN]

  Cancel all pending print jobs and restart the print service.

  [ACTION:Fix it]
  `;

    const result = parseResponse(raw);
    expect(result.type).toBe("action");
    if (result.type === "action") {
      expect(result.situation).toBe(
        "Your printer queue is stuck with 3 jobs.",
      );
      expect(result.plan).toBe(
        "Cancel all pending print jobs and restart the print service.",
      );
    }
  });

  // ── Done cards ──

  it("parses a done response", () => {
    const raw = `[DONE]
Connected to "Alex's iPhone" hotspot. Verified — internet is working.`;

    const result = parseResponse(raw);
    expect(result).toEqual({
      type: "done",
      summary: `Connected to "Alex's iPhone" hotspot. Verified — internet is working.`,
    });
  });

  it("parses multi-line done response", () => {
    const raw = `[DONE]
Flushed DNS cache successfully.
Verified: google.com now resolves to 142.250.80.46.`;

    const result = parseResponse(raw);
    expect(result.type).toBe("done");
    if (result.type === "done") {
      expect(result.summary).toContain("Flushed DNS cache");
      expect(result.summary).toContain("142.250.80.46");
    }
  });

  // ── Info cards ──

  it("parses an info response", () => {
    const raw = `[INFO]
Your Wi-Fi is connected to "HomeNetwork" at 45 Mbps. Everything looks normal.`;

    const result = parseResponse(raw);
    expect(result).toEqual({
      type: "info",
      summary: `Your Wi-Fi is connected to "HomeNetwork" at 45 Mbps. Everything looks normal.`,
    });
  });

  // ── Fallback to plain text ──

  it("falls back to text for unstructured responses", () => {
    const raw = "I checked your system and everything looks fine.";

    const result = parseResponse(raw);
    expect(result).toEqual({
      type: "text",
      content: "I checked your system and everything looks fine.",
    });
  });

  it("falls back to text for empty string", () => {
    const result = parseResponse("");
    expect(result).toEqual({ type: "text", content: "" });
  });

  it("falls back to text when markers are incomplete", () => {
    const raw = `[SITUATION]
Something is wrong but no plan follows.`;

    const result = parseResponse(raw);
    // No [PLAN] or [ACTION], so doesn't match action pattern.
    // Also not [DONE] or [INFO], so falls back to text.
    expect(result.type).toBe("text");
  });

  // ── Edge cases ──

  it("handles markers with no content gracefully", () => {
    const raw = `[INFO]
`;
    // The regex requires [\s\S]+ (one or more chars), so empty content won't match
    const result = parseResponse(raw);
    expect(result.type).toBe("text");
  });

  it("only matches first occurrence of action pattern", () => {
    const raw = `[SITUATION]
First problem.
[PLAN]
First fix.
[ACTION:Fix]

Some extra text after.`;

    const result = parseResponse(raw);
    expect(result.type).toBe("action");
    if (result.type === "action") {
      expect(result.situation).toBe("First problem.");
      expect(result.plan).toBe("First fix.");
      expect(result.actionLabel).toBe("Fix");
    }
  });

  it("parses SPA json payload with prefixed prose", () => {
    const raw = `I checked your system.
{
  "kind":"spa",
  "situation":"CPU is high",
  "plan":"Stop heavy app",
  "action":{"label":"Stop App","type":"RUN_STEP"}
}`;
    const result = parseResponse(raw);
    expect(result.type).toBe("action");
    if (result.type === "action") {
      expect(result.situation).toBe("CPU is high");
      expect(result.plan).toBe("Stop heavy app");
      expect(result.actionLabel).toBe("Stop App");
      expect(result.actionType).toBe("RUN_STEP");
    }
  });
});
