export type ParsedResponse =
  | { type: "action"; situation: string; plan: string; actionLabel: string }
  | { type: "done"; summary: string }
  | { type: "info"; summary: string }
  | {
      type: "credentials_collected";
      reference: string;
      provider: string;
      chatChannel?: string;
      summary: string;
    }
  | { type: "text"; content: string };

/**
 * Parse a structured LLM response into a typed object.
 *
 * The LLM uses bracket markers: [SITUATION], [PLAN], [ACTION:Label], [DONE], [INFO].
 * If no markers are found, falls back to { type: "text" }.
 */
export function parseResponse(raw: string): ParsedResponse {
  // Strip any <think>...</think> blocks (some models emit reasoning tags).
  const trimmed = raw.replace(/<think>[\s\S]*?<\/think>\s*/g, "").trim();

  // Action card: [SITUATION]...[PLAN]...[ACTION:Label]
  const actionMatch = trimmed.match(
    /\[SITUATION\]\s*([\s\S]*?)\s*\[PLAN\]\s*([\s\S]*?)\s*\[ACTION:([^\]]+)\]/,
  );
  if (actionMatch) {
    return {
      type: "action",
      situation: actionMatch[1].trim(),
      plan: actionMatch[2].trim(),
      actionLabel: actionMatch[3].trim(),
    };
  }

  // Done card: [DONE]...
  const doneMatch = trimmed.match(/\[DONE\]\s*([\s\S]+)/);
  if (doneMatch) {
    return {
      type: "done",
      summary: doneMatch[1].trim(),
    };
  }

  // Info card: [INFO]...
  const infoMatch = trimmed.match(/\[INFO\]\s*([\s\S]+)/);
  if (infoMatch) {
    return {
      type: "info",
      summary: infoMatch[1].trim(),
    };
  }

  // Credential artifact: [CREDENTIALS_COLLECTED]...
  const credMatch = trimmed.match(
    /\[CREDENTIALS_COLLECTED\]\s*([\s\S]+)/,
  );
  if (credMatch) {
    const body = credMatch[1].trim();
    const ref =
      body.match(/Reference:\s*([^\n]+)/i)?.[1]?.trim() ||
      body.match(/Ref:\s*([^\n]+)/i)?.[1]?.trim() ||
      "unknown";
    const provider =
      body.match(/Provider:\s*([^\n]+)/i)?.[1]?.trim() || "Unknown";
    const chatChannel = body.match(/Chat channel:\s*([^\n]+)/i)?.[1]?.trim();

    return {
      type: "credentials_collected",
      reference: ref,
      provider,
      chatChannel: chatChannel && chatChannel.toLowerCase() !== "none"
        ? chatChannel
        : undefined,
      summary: body,
    };
  }

  // Fallback: plain text
  return { type: "text", content: trimmed };
}
