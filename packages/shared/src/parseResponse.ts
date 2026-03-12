export type ParsedResponse =
  | {
      type: "action";
      situation: string;
      plan?: string;
      actionLabel: string;
      actionType?: string;
    }
  | {
      type: "user_question";
      questions: Array<{
        question: string;
        header: string;
        options?: Array<{ label: string; description: string }>;
        text_input?: { placeholder?: string; default?: string };
        secure_input?: { placeholder?: string; secret_name: string };
        multiSelect?: boolean;
      }>;
    }
  | { type: "done"; summary: string }
  | { type: "info"; summary: string }
  | { type: "text"; content: string };

/**
 * Parse a structured LLM response into a typed object.
 *
 * Tries JSON first (ui_* tool call payloads), then falls back to
 * legacy bracket markers: [SITUATION], [PLAN], [ACTION:Label], [DONE], [INFO].
 */
export function parseResponse(raw: string): ParsedResponse {
  // Strip any <think>...</think> blocks (some models emit reasoning tags).
  const trimmed = raw.replace(/<think>[\s\S]*?<\/think>\s*/g, "").trim();

  // JSON SPA / user_question payloads (optionally with prefixed prose)
  {
    const candidate = (() => {
      const start = trimmed.indexOf("{");
      const end = trimmed.lastIndexOf("}");
      if (start === -1 || end === -1 || end <= start) return null;
      return trimmed.slice(start, end + 1);
    })();

    if (candidate) {
      try {
        const obj = JSON.parse(candidate) as {
          kind?: string;
          summary?: string;
          situation?: string;
          plan?: string;
          action?: {
            label?: string;
            type?: string;
          };
          questions?: Array<{
            question: string;
            header: string;
            options: Array<{ label: string; description: string }>;
            multiSelect?: boolean;
          }>;
        };

        const kind = (obj.kind || "").toLowerCase();
        if (
          kind === "spa" &&
          obj.situation &&
          obj.action?.label
        ) {
          return {
            type: "action",
            situation: obj.situation,
            plan: obj.plan,
            actionLabel: obj.action.label,
            actionType: obj.action.type,
          };
        }
        if (kind === "user_question" && Array.isArray(obj.questions)) {
          return {
            type: "user_question",
            questions: obj.questions.map((q) => ({
              question: q.question,
              header: q.header,
              options: q.options,
              multiSelect: Boolean(q.multiSelect),
            })),
          };
        }
        if (kind === "done" && obj.summary) {
          return { type: "done", summary: obj.summary };
        }
        if (kind === "info" && obj.summary) {
          return { type: "info", summary: obj.summary };
        }
      } catch {
        // ignore and continue with legacy marker parsing
      }
    }
  }

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

  // Fallback: plain text
  return { type: "text", content: trimmed };
}
