import { File } from "expo-file-system";
import { getProxyHeaders, getProxyUrl } from "./auth";

function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

const MODEL = "claude-sonnet-4-20250514";

export interface TriageResult {
  analysis: string;
  model: string;
}

/**
 * Send a photo to Claude Vision for IT triage analysis.
 * Routes through the Noah proxy using Bearer token auth.
 */
export async function analyzePhoto(photoUri: string): Promise<TriageResult> {
  const headers = await getProxyHeaders();
  if (!headers.Authorization) {
    throw new Error("Not authenticated. Please sign in first.");
  }

  // Read image as base64
  const file = new File(photoUri);
  const buffer = await file.arrayBuffer();
  const base64 = arrayBufferToBase64(buffer);

  // Detect media type from URI
  const ext = photoUri.split(".").pop()?.toLowerCase() ?? "jpeg";
  const mediaType =
    ext === "png"
      ? "image/png"
      : ext === "webp"
        ? "image/webp"
        : ext === "gif"
          ? "image/gif"
          : "image/jpeg";

  const proxyUrl = getProxyUrl();
  const response = await fetch(`${proxyUrl}/v1/messages`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...headers,
    },
    body: JSON.stringify({
      model: MODEL,
      max_tokens: 1024,
      system:
        "You are Noah, an IT support assistant. The user photographed a screen or device showing an issue. " +
        "Identify the problem, explain what's happening in plain language, and suggest fixes the user can try. " +
        "If the fix requires running commands or changing system settings on a computer, mention that pairing " +
        "with Noah Desktop would enable automatic remediation. Keep your response concise and actionable.",
      messages: [
        {
          role: "user",
          content: [
            {
              type: "image",
              source: {
                type: "base64",
                media_type: mediaType,
                data: base64,
              },
            },
            {
              type: "text",
              text: "What issue does this photo show? How can I fix it?",
            },
          ],
        },
      ],
    }),
  });

  if (!response.ok) {
    const body = await response.text();
    if (response.status === 401) {
      throw new Error("Session expired. Please sign in again.");
    }
    if (response.status === 429) {
      throw new Error("Rate limited. Please wait a moment and try again.");
    }
    throw new Error(`API error (${response.status}): ${body.slice(0, 200)}`);
  }

  const data = (await response.json()) as {
    content: Array<{ type: string; text?: string }>;
    model: string;
  };

  const text = data.content
    .filter((b) => b.type === "text" && b.text)
    .map((b) => b.text)
    .join("\n\n");

  if (!text) {
    throw new Error("Claude returned an empty response.");
  }

  return { analysis: text, model: data.model };
}
