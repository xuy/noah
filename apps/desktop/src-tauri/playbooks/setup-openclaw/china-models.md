---
name: setup-openclaw/china-models
description: Set up Chinese AI model providers for OpenClaw (Volcano Engine, Moonshot, DeepSeek, Qwen, GLM)
platform: all
last_reviewed: 2026-03-08
author: noah-team
source: bundled
emoji: 🦞
---

# Chinese Model Providers

Guide for setting up Chinese AI model providers with OpenClaw. These are
useful for users in China where Anthropic/OpenAI APIs may be slow or
unavailable, or who prefer domestic models.

## Step 1: Choose Provider

Ask the user which provider they want to use:

### Volcano Engine (火山引擎 / 豆包)
- Provider ID: `volcengine`
- Models: Doubao Seed 1.8, Kimi K2.5, GLM 4.7, DeepSeek V3.2
- Has separate coding-optimized models under `volcengine-plan`
- API key env var: `VOLCANO_ENGINE_API_KEY`
- Sign up: https://www.volcengine.com/

### BytePlus (International alternative to Volcano Engine)
- Provider ID: `byteplus`
- Same models as Volcano Engine, for international users
- API key env var: `BYTEPLUS_API_KEY`

### Moonshot AI (月之暗面 / Kimi)
- Uses OpenAI-compatible endpoint
- Models: Kimi K2.5, K2 Turbo Preview, thinking variants
- API key env var: `MOONSHOT_API_KEY`
- Base URL: `https://api.moonshot.ai/v1`
- Sign up: https://platform.moonshot.cn/

### Z.AI / GLM (智谱 AI)
- Provider ID: `zai`
- Models: GLM-5 and variants
- API key env var: `ZAI_API_KEY`
- Sign up: https://open.bigmodel.cn/

### Qwen Portal (通义千问 — 免费)
- Free tier via OAuth (no API key needed)
- Models: Qwen Coder + Vision
- Auth via device code flow (see Step 3)

### DeepSeek (深度求索)
- Uses OpenAI-compatible endpoint
- Models: DeepSeek V3, DeepSeek Coder
- API key env var: `DEEPSEEK_API_KEY`
- Base URL: `https://api.deepseek.com/v1`
- Sign up: https://platform.deepseek.com/

## Step 2: Get API Key

For most providers, the user needs an API key. Collect it via `secure_input`.

Then set it in the OpenClaw config:
```
openclaw config set env.<ENV_VAR_NAME> "<api_key>"
```

For example, for Volcano Engine:
```
openclaw config set env.VOLCANO_ENGINE_API_KEY "<key>"
```

## Step 3: Configure the Model

**For built-in providers** (volcengine, byteplus, zai):
```
openclaw config set agents.defaults.model.primary "volcengine/doubao-seed-1.8"
```

**For Qwen Portal** (free, OAuth):
```
openclaw models auth login --provider qwen-portal --set-default
```
This opens a device code flow — use WAIT_FOR_USER.

**For OpenAI-compatible providers** (Moonshot, DeepSeek):
These need a custom provider entry in `~/.openclaw/openclaw.json`:
```json5
{
  models: {
    providers: {
      moonshot: {
        baseUrl: "https://api.moonshot.ai/v1",
        apiKey: "${MOONSHOT_API_KEY}",
        api: "openai",
        models: [
          { id: "kimi-k2.5" },
          { id: "kimi-k2-turbo-preview" }
        ]
      }
    }
  },
  agents: {
    defaults: {
      model: {
        primary: "moonshot/kimi-k2.5"
      }
    }
  }
}
```

For DeepSeek:
```json5
{
  models: {
    providers: {
      deepseek: {
        baseUrl: "https://api.deepseek.com/v1",
        apiKey: "${DEEPSEEK_API_KEY}",
        api: "openai",
        models: [
          { id: "deepseek-chat" },
          { id: "deepseek-coder" }
        ]
      }
    }
  },
  agents: {
    defaults: {
      model: {
        primary: "deepseek/deepseek-chat"
      }
    }
  }
}
```

## Step 4: Verify

Check that the model is accessible:
```
openclaw models status
```

Send a test message through a connected channel to confirm the model responds.

If rate-limited (429 errors), the provider may require a paid plan or the
model may need switching. Check `openclaw logs --follow` for details.

## Step 5: Optional — Add Fallback Models

For reliability, configure fallback models from a different provider:
```json5
{
  agents: {
    defaults: {
      model: {
        primary: "volcengine/doubao-seed-1.8",
        fallbacks: ["moonshot/kimi-k2.5", "deepseek/deepseek-chat"]
      }
    }
  }
}
```

## Tools referenced
- `shell_run` — openclaw CLI commands, config edits
- `ui_user_question` with options — provider selection
- `ui_user_question` with `secure_input` — API keys
- `ui_spa` with WAIT_FOR_USER — OAuth flows (Qwen Portal)
