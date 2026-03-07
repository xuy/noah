use crate::agent::llm_client::{ContentBlock, Message, MessageContent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenclawStage {
    InstallCheck,
    PrimaryProviderCapture,
    PrimaryProviderVerify,
    ChannelCapture,
    ChannelVerify,
    Done,
}

impl OpenclawStage {
    pub fn as_str(self) -> &'static str {
        match self {
            OpenclawStage::InstallCheck => "INSTALL_CHECK",
            OpenclawStage::PrimaryProviderCapture => "PRIMARY_PROVIDER_CAPTURE",
            OpenclawStage::PrimaryProviderVerify => "PRIMARY_PROVIDER_VERIFY",
            OpenclawStage::ChannelCapture => "CHANNEL_CAPTURE",
            OpenclawStage::ChannelVerify => "CHANNEL_VERIFY",
            OpenclawStage::Done => "DONE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenclawContext {
    pub stage: OpenclawStage,
    pub provider: Option<String>,
    pub channel: Option<String>,
    pub credential_ref: Option<String>,
}

fn extract_text(content: &MessageContent) -> Option<&str> {
    match content {
        MessageContent::Text(t) => Some(t.as_str()),
        MessageContent::Blocks(_) => None,
    }
}

fn parse_labeled_value(haystack: &str, label: &str) -> Option<String> {
    for line in haystack.lines() {
        let trimmed = line.trim();
        if trimmed
            .to_lowercase()
            .starts_with(&format!("{}:", label.to_lowercase()))
        {
            let v = trimmed.split_once(':')?.1.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn infer_provider_from_text(haystack: &str) -> Option<String> {
    let lower = haystack.to_lowercase();
    if lower.contains("anthropic") || lower.contains("claude") {
        return Some("Anthropic".to_string());
    }
    if lower.contains("openai") || lower.contains("gpt") {
        return Some("OpenAI".to_string());
    }
    if lower.contains("openrouter") {
        return Some("OpenRouter".to_string());
    }
    if lower.contains("gemini") || lower.contains("google") {
        return Some("Google Gemini".to_string());
    }
    None
}

fn infer_channel_from_text(haystack: &str) -> Option<String> {
    let lower = haystack.to_lowercase();
    if lower.contains("telegram") {
        return Some("Telegram".to_string());
    }
    if lower.contains("discord") {
        return Some("Discord".to_string());
    }
    None
}

pub fn parse_openclaw_context(messages: &[Message]) -> OpenclawContext {
    let mut install_checked = false;
    let mut provider_verified = false;
    let mut channel_verified = false;
    let mut provider: Option<String> = None;
    let mut channel: Option<String> = None;
    let mut credential_ref: Option<String> = None;

    for message in messages {
        if let Some(text) = extract_text(&message.content) {
            let lower = text.to_lowercase();
            if credential_ref.is_none() && lower.contains("credential reference: openclaw-") {
                credential_ref = parse_labeled_value(text, "Credential reference");
            }
            if provider.is_none() {
                provider = parse_labeled_value(text, "Provider");
            }
            if channel.is_none() {
                channel = parse_labeled_value(text, "Chat channel");
            }
            if provider.is_none() {
                provider = infer_provider_from_text(text);
            }
            if channel.is_none() {
                channel = infer_channel_from_text(text);
            }
            if lower.contains("openclaw --version") || lower.contains("openclaw is installed") {
                install_checked = true;
            }
            if lower.contains("provider verified")
                || lower.contains("provider connection is working")
                || lower.contains("provider connection verified")
            {
                provider_verified = true;
            }
            if lower.contains("telegram bot connection verified")
                || lower.contains("discord connection verified")
                || lower.contains("channel verification passed")
            {
                channel_verified = true;
            }
        }
        if let MessageContent::Blocks(blocks) = &message.content {
            for b in blocks {
                if let ContentBlock::ToolUse { name, input, .. } = b {
                    if name == "shell_run" {
                        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                            let cmd_l = cmd.to_lowercase();
                            if cmd_l.contains("openclaw --version") {
                                install_checked = true;
                            }
                            if cmd_l.contains("openclaw doctor") && !cmd_l.contains("--fix") {
                                if credential_ref.is_some() {
                                    provider_verified = true;
                                }
                            }
                            if cmd_l.contains("telegram") || cmd_l.contains("discord") {
                                channel_verified = true;
                            }
                        }
                    }
                }
            }
        }
    }

    let stage = if !install_checked {
        OpenclawStage::InstallCheck
    } else if credential_ref.is_none() {
        OpenclawStage::PrimaryProviderCapture
    } else if !provider_verified {
        OpenclawStage::PrimaryProviderVerify
    } else if channel
        .as_deref()
        .is_some_and(|c| !c.eq_ignore_ascii_case("none"))
        && !channel_verified
    {
        OpenclawStage::ChannelCapture
    } else if channel.is_some() && !channel_verified {
        OpenclawStage::ChannelVerify
    } else {
        OpenclawStage::Done
    };

    OpenclawContext {
        stage,
        provider,
        channel,
        credential_ref,
    }
}

pub fn openclaw_stage_overlay(ctx: &OpenclawContext) -> String {
    let mut out = format!(
        "\n\n## OpenClaw Stage Machine\n\
Current stage: `{}`.\n\
Treat this as the skeleton. Keep natural, human guidance as the conversational layer.\n",
        ctx.stage.as_str()
    );
    match ctx.stage {
        OpenclawStage::InstallCheck => {
            out.push_str(
                "- Goal: confirm OpenClaw install and version.\n- Next: move to PRIMARY_PROVIDER_CAPTURE.\n",
            );
        }
        OpenclawStage::PrimaryProviderCapture => {
            out.push_str(
                "- Goal: capture primary model provider credentials through Noah secure credential form.\n\
- UX: suggest two common channels (Telegram, Discord), but explicitly allow user to choose another channel.\n\
- Do not ask for secrets in plain chat text.\n\
- Memory/embedding provider is optional at this stage. Do not block setup on memory provider.\n",
            );
            if let Some(provider) = &ctx.provider {
                match provider.to_lowercase().as_str() {
                    "anthropic" => out.push_str(
                        "- Explain where to get Anthropic API key in plain language: Anthropic Console (console.anthropic.com -> Account Settings/API Keys).\n\
- Mention alternative for Anthropic subscription users: `claude setup-token` (if they prefer setup-token auth).\n",
                    ),
                    "openai" => out.push_str(
                        "- Explain where to get OpenAI API key in plain language: OpenAI platform API keys page (platform.openai.com/api-keys).\n",
                    ),
                    "openrouter" => out.push_str(
                        "- Explain where to get OpenRouter key in plain language: OpenRouter dashboard keys page.\n",
                    ),
                    _ => {}
                }
            } else {
                out.push_str(
                    "- If provider is unknown, ask one plain-language provider choice question (OpenAI or Anthropic as common defaults).\n",
                );
            }
        }
        OpenclawStage::PrimaryProviderVerify => {
            out.push_str(
                "- Goal: verify primary provider works.\n\
- Use read-only checks first (e.g., doctor/health). Avoid filesystem surgery and avoid `doctor --fix` loops.\n\
- If credential appears test/dummy/invalid, explain clearly and keep user in controlled retry path.\n\
- Memory/embedding issues are non-blocking; defer them.\n",
            );
            if ctx.credential_ref.is_some() {
                out.push_str(
                    "- A secure credential reference already exists. Do not ask user to 'apply stored credentials' generically.\n\
- Run verification checks directly.\n\
- If verification fails, provide one explicit reason and ask user to re-save a real key in Noah secure form.\n",
                );
            }
        }
        OpenclawStage::ChannelCapture => {
            out.push_str(
                "- Goal: capture optional chat channel token securely.\n\
- Keep channel setup optional and skippable.\n\
- If user picks Telegram/Discord, provide concise token acquisition guidance.\n\
- Telegram guidance: talk to @BotFather, run /newbot, copy token.\n\
- Discord guidance: Developer Portal -> create app/bot -> copy bot token.\n\
- If user picks another channel, adapt guidance instead of refusing.\n",
            );
        }
        OpenclawStage::ChannelVerify => {
            out.push_str("- Goal: verify optional channel integration or mark as pending.\n");
        }
        OpenclawStage::Done => {
            out.push_str("- Goal reached: required stages complete. [DONE] is allowed.\n");
        }
    }
    if let Some(p) = &ctx.provider {
        out.push_str(&format!("- Provider: {}\n", p));
    }
    if let Some(ch) = &ctx.channel {
        out.push_str(&format!("- Channel: {}\n", ch));
    }
    if let Some(cref) = &ctx.credential_ref {
        out.push_str(&format!("- Credential reference: {}\n", cref));
    }
    out
}

pub fn blocked_openclaw_shell_command(stage: OpenclawStage, command: &str) -> Option<&'static str> {
    let lower = command.trim().to_lowercase();

    let hard_blocks = [
        "openclaw doctor --fix",
        "mkdir -p ~/.openclaw/agents/main/agent",
        "auth-profiles.json",
        "openclaw config set agents.defaults.memorysearch.provider",
    ];
    if hard_blocks.iter().any(|p| lower.contains(p)) {
        return Some("blocked_loop_or_manual_schema_surgery");
    }

    if stage == OpenclawStage::PrimaryProviderVerify
        && (lower.contains("openclaw memory status")
            || lower.contains("memorysearch.provider")
            || lower.contains("embedding"))
    {
        return Some("memory_provider_is_optional_in_primary_verify");
    }

    None
}

pub fn has_disallowed_openclaw_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    if lower.contains("openclaw configure") {
        return true;
    }
    if lower.contains("secure credential form is not available")
        || lower.contains("secure credential form isn't available")
    {
        return true;
    }
    if !lower.contains("openclaw config") {
        return false;
    }

    let allowed = [
        "openclaw config show",
        "openclaw config get",
        "openclaw config file",
        "openclaw config validate",
        "openclaw config --help",
    ];
    !allowed.iter().any(|pat| lower.contains(pat))
}

pub fn missing_openclaw_action_format(text: &str) -> bool {
    let has_action = text.contains("[SITUATION]") && text.contains("[PLAN]") && text.contains("[ACTION:");
    let has_done = text.contains("[DONE]");
    !has_action && !has_done
}

pub fn has_awkward_provider_shorthand(text: &str) -> bool {
    let lower = text.to_lowercase();
    (lower.contains("`openai` - for") || lower.contains("openai - for"))
        || (lower.contains("`anthropic` - for") || lower.contains("anthropic - for"))
        || (lower.contains("`openrouter` - access") || lower.contains("openrouter - access"))
}

pub fn has_vague_apply_credentials_loop(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("apply your stored credentials")
        || lower.contains("apply the credentials now")
        || lower.contains("confirm when ready")
}

fn asks_where_to_get_key(user_message: &str) -> bool {
    let lower = user_message.to_lowercase();
    (lower.contains("where") || lower.contains("find") || lower.contains("get"))
        && (lower.contains("api key") || lower.contains("credential") || lower.contains("token"))
}

pub fn missing_provider_source_guidance(
    user_message: &str,
    candidate_text: &str,
    provider: Option<&str>,
) -> bool {
    if !asks_where_to_get_key(user_message) {
        return false;
    }
    let resp = candidate_text.to_lowercase();
    match provider.unwrap_or("").to_lowercase().as_str() {
        "anthropic" => !(resp.contains("console.anthropic.com") || resp.contains("anthropic console")),
        "openai" => !(resp.contains("platform.openai.com/api-keys") || resp.contains("openai api keys")),
        "openrouter" => !(resp.contains("openrouter") && resp.contains("key")),
        _ => !(
            resp.contains("console.anthropic.com")
                || resp.contains("platform.openai.com/api-keys")
                || resp.contains("api keys")
        ),
    }
}
