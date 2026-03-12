import * as SecureStore from "expo-secure-store";
import * as FileSystem from "expo-file-system";

const API_KEY_STORAGE_KEY = "noah_api_key";
const API_URL = "https://api.anthropic.com/v1/messages";
const MODEL = "claude-sonnet-4-20250514";

export async function getApiKey(): Promise<string | null> {
  try {
    return await SecureStore.getItemAsync(API_KEY_STORAGE_KEY);
  } catch {
    return null;
  }
}

export interface TriageResult {
  analysis: string;
  model: string;
}

/**
 * Send a photo to Claude Vision for IT triage analysis.
 * Reads the image from disk, base64-encodes it, and calls the Messages API.
 */
export async function analyzePhoto(photoUri: string): Promise<TriageResult> {
  const apiKey = await getApiKey();
  if (!apiKey) {
    throw new Error("No API key configured. Go to Settings to add one.");
  }

  // Read image as base64
  const base64 = await FileSystem.readAsStringAsync(photoUri, {
    encoding: FileSystem.EncodingType.Base64,
  });

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

  const response = await fetch(API_URL, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-api-key": apiKey,
      "anthropic-version": "2023-06-01",
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
      throw new Error("Invalid API key. Check your key in Settings.");
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
