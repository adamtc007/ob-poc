//! Claude Code CLI client.
//!
//! This backend is intended for local smoke tests and developer sessions where
//! Claude Code/Zed auth is available but a first-party Anthropic API key is not.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::Value;

use super::llm_client::{LlmClient, ToolCallResult, ToolDefinition};

const DEFAULT_MODEL: &str = "sonnet";
const DEFAULT_MAX_BUDGET_USD: &str = "0.50";

#[derive(Clone)]
pub struct ClaudeCodeCliClient {
    bin: String,
    model: String,
    max_budget_usd: String,
    preserve_anthropic_api_key: bool,
}

impl ClaudeCodeCliClient {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            bin: std::env::var("CLAUDE_CODE_CLI_BIN").unwrap_or_else(|_| "claude".to_string()),
            model: std::env::var("CLAUDE_CODE_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            max_budget_usd: std::env::var("CLAUDE_CODE_MAX_BUDGET_USD")
                .unwrap_or_else(|_| DEFAULT_MAX_BUDGET_USD.to_string()),
            preserve_anthropic_api_key: env_truthy("CLAUDE_CODE_CLI_PRESERVE_ANTHROPIC_API_KEY"),
        })
    }

    fn invoke_text(&self, prompt: String, schema: Option<Value>) -> Result<String> {
        let mut command = Command::new(&self.bin);
        command
            .arg("-p")
            .arg("--model")
            .arg(&self.model)
            .arg("--output-format")
            .arg("json")
            .arg("--no-session-persistence")
            .arg("--max-budget-usd")
            .arg(&self.max_budget_usd)
            .arg("--tools")
            .arg("")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(schema) = schema {
            command
                .arg("--json-schema")
                .arg(serde_json::to_string(&schema).context("serialize Claude Code JSON schema")?);
        }

        if !self.preserve_anthropic_api_key {
            command.env_remove("ANTHROPIC_API_KEY");
        }

        let mut child = command
            .spawn()
            .with_context(|| format!("spawn Claude Code CLI '{}'", self.bin))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Claude Code CLI stdin unavailable"))?;
        stdin
            .write_all(prompt.as_bytes())
            .context("write Claude Code CLI prompt")?;
        drop(stdin);

        let output = child
            .wait_with_output()
            .context("wait for Claude Code CLI")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let envelope = parse_cli_envelope(stdout.trim())
            .with_context(|| compact_cli_error(output.status.code(), &stdout, &stderr))?;
        if !output.status.success() || cli_envelope_is_error(&envelope) {
            return Err(anyhow!(
                "{}",
                compact_cli_envelope_error(&envelope, &stderr)
            ));
        }

        result_text_from_envelope(&envelope)
    }

    fn invoke_tool(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tool: &ToolDefinition,
    ) -> Result<ToolCallResult> {
        let schema = tool.parameters.clone();
        let prompt = format!(
            "{system_prompt}\n\nUser request:\n{user_prompt}\n\nReturn only the JSON arguments for the virtual tool `{}`. Do not wrap the response in `tool_name`, `arguments`, markdown, or prose.",
            tool.name
        );
        let result_text = self.invoke_text(prompt, Some(schema))?;
        let value = parse_result_json(&result_text)?;
        tool_call_from_value(value, tool)
    }
}

#[async_trait]
impl LlmClient for ClaudeCodeCliClient {
    async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let prompt = format!("{system_prompt}\n\nUser request:\n{user_prompt}");
        let client = self.clone();
        tokio::task::spawn_blocking(move || client.invoke_text(prompt, None))
            .await
            .context("join Claude Code CLI chat task")?
    }

    async fn chat_json(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let prompt =
            format!("{system_prompt}\n\nUser request:\n{user_prompt}\n\nReturn valid JSON only.");
        let schema = serde_json::json!({ "type": "object" });
        let client = self.clone();
        tokio::task::spawn_blocking(move || client.invoke_text(prompt, Some(schema)))
            .await
            .context("join Claude Code CLI JSON task")?
    }

    async fn chat_with_tool(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tool: &ToolDefinition,
    ) -> Result<ToolCallResult> {
        let client = self.clone();
        let system_prompt = system_prompt.to_string();
        let user_prompt = user_prompt.to_string();
        let tool = tool.clone();
        tokio::task::spawn_blocking(move || client.invoke_tool(&system_prompt, &user_prompt, &tool))
            .await
            .context("join Claude Code CLI tool task")?
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn provider_name(&self) -> &str {
        "ClaudeCodeCli"
    }
}

fn env_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn parse_cli_envelope(stdout: &str) -> Result<Value> {
    serde_json::from_str(stdout).context("parse Claude Code CLI JSON envelope")
}

fn cli_envelope_is_error(envelope: &Value) -> bool {
    envelope
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || envelope
            .get("subtype")
            .and_then(Value::as_str)
            .map(|subtype| subtype.starts_with("error"))
            .unwrap_or(false)
}

fn result_text_from_envelope(envelope: &Value) -> Result<String> {
    match envelope.get("result") {
        Some(Value::String(text)) => Ok(text.clone()),
        Some(value) => Ok(value.to_string()),
        None => Err(anyhow!("Claude Code CLI result field missing")),
    }
}

fn compact_cli_error(status_code: Option<i32>, stdout: &str, stderr: &str) -> String {
    let mut message =
        format!("Claude Code CLI failed before returning a JSON envelope; status={status_code:?}");
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        message.push_str("; stderr=");
        message.push_str(&truncate(stderr, 500));
    }
    let stdout = stdout.trim();
    if !stdout.is_empty() {
        message.push_str("; stdout=");
        message.push_str(&truncate(stdout, 500));
    }
    message
}

fn compact_cli_envelope_error(envelope: &Value, stderr: &str) -> String {
    let subtype = envelope
        .get("subtype")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let result = envelope
        .get("result")
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| value.to_string())
        })
        .unwrap_or_default();
    let mut message = format!(
        "Claude Code CLI error {subtype}: {}",
        truncate(&result, 500)
    );
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        message.push_str("; stderr=");
        message.push_str(&truncate(stderr, 300));
    }
    message
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for ch in value.chars().take(max_chars) {
        output.push(ch);
    }
    if value.chars().count() > max_chars {
        output.push_str("...");
    }
    output
}

fn parse_result_json(text: &str) -> Result<Value> {
    let trimmed = text.trim();
    serde_json::from_str(trimmed)
        .or_else(|_| {
            let start = trimmed
                .find('{')
                .ok_or_else(|| anyhow!("Claude Code CLI result did not contain JSON object"))?;
            let end = trimmed
                .rfind('}')
                .ok_or_else(|| anyhow!("Claude Code CLI result did not contain JSON object"))?;
            serde_json::from_str(&trimmed[start..=end]).context("parse embedded JSON object")
        })
        .context("parse Claude Code CLI result JSON")
}

fn tool_call_from_value(value: Value, tool: &ToolDefinition) -> Result<ToolCallResult> {
    let arguments = if value
        .get("tool_name")
        .and_then(Value::as_str)
        .map(|tool_name| tool_name == tool.name)
        .unwrap_or(false)
    {
        value
            .get("arguments")
            .cloned()
            .ok_or_else(|| anyhow!("Claude Code CLI arguments missing"))?
    } else {
        value
    };
    Ok(ToolCallResult {
        tool_name: tool.name.clone(),
        arguments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_successful_cli_tool_result() {
        let envelope = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "result": "{\"verb\":\"kyc-case.update-status\"}"
        });
        let text = result_text_from_envelope(&envelope).unwrap();
        let value = parse_result_json(&text).unwrap();
        let tool = ToolDefinition {
            name: "draft".to_string(),
            description: "draft".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "verb": { "type": "string" }
                }
            }),
        };
        let result = tool_call_from_value(value, &tool).unwrap();
        assert_eq!(result.tool_name, "draft");
        assert_eq!(result.arguments["verb"], "kyc-case.update-status");
    }

    #[test]
    fn tolerates_wrapped_cli_tool_result() {
        let tool = ToolDefinition {
            name: "draft".to_string(),
            description: "draft".to_string(),
            parameters: serde_json::json!({ "type": "object" }),
        };
        let result = tool_call_from_value(
            serde_json::json!({
                "tool_name": "draft",
                "arguments": {
                    "verb": "kyc-case.update-status"
                }
            }),
            &tool,
        )
        .unwrap();
        assert_eq!(result.tool_name, "draft");
        assert_eq!(result.arguments["verb"], "kyc-case.update-status");
    }

    #[test]
    fn detects_cli_error_envelope() {
        let envelope = serde_json::json!({
            "subtype": "error_max_budget_usd",
            "is_error": false,
            "result": ""
        });
        assert!(cli_envelope_is_error(&envelope));
    }
}
