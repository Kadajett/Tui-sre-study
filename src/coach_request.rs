use super::budget::Budget;
use anyhow::{ensure, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, io::Read, time::Duration};

#[derive(Deserialize)]
struct Completion {
    choices: Vec<Choice>,
}
#[derive(Deserialize)]
struct Choice {
    message: Message,
}
#[derive(Deserialize)]
pub(super) struct Message {
    pub(super) content: Option<String>,
    #[serde(default)]
    pub(super) tool_calls: Vec<ToolCall>,
}
#[derive(Deserialize)]
pub(super) struct ToolCall {
    pub(super) id: String,
    pub(super) function: ToolFunction,
}
#[derive(Deserialize)]
pub(super) struct ToolFunction {
    pub(super) name: String,
    pub(super) arguments: String,
}

pub(super) struct RequestOptions<'a> {
    pub(super) tools: bool,
    pub(super) teaching: bool,
    pub(super) budget: &'a Budget,
}
pub(super) fn request(
    client: &Client,
    key: &str,
    messages: &[Value],
    mode: RequestOptions<'_>,
) -> Result<Message> {
    let transport = Transport {
        client,
        key,
        url: "https://openrouter.ai/api/v1/chat/completions",
    };
    request_at(&transport, messages, mode)
}

struct Transport<'a> {
    client: &'a Client,
    key: &'a str,
    url: &'a str,
}

fn request_at(
    transport: &Transport<'_>,
    messages: &[Value],
    mode: RequestOptions<'_>,
) -> Result<Message> {
    let body = request_body(messages, &mode);
    for attempt in 0..3 {
        mode.budget.phase(u8::from(attempt > 0));
        let message = request_once(transport, &body, mode.budget)?;
        if usable(&message, mode.teaching) {
            return Ok(message);
        }
    }
    anyhow::bail!(
        "Mercury returned empty or invalid replies after 3 attempts. Please try your message again"
    )
}

fn usable(message: &Message, teaching: bool) -> bool {
    if !message.tool_calls.is_empty() {
        return true;
    }
    let Some(text) = message
        .content
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    else {
        return false;
    };
    !teaching || crate::teaching_protocol::TeachingReply::parse(text).is_ok()
}

fn request_body(messages: &[Value], mode: &RequestOptions<'_>) -> Value {
    let mut body = json!({"model":env::var("OPENROUTER_MODEL").unwrap_or_else(|_|"inception/mercury-2".into()), "messages":messages, "max_tokens":2000,"reasoning":{"effort":"low","exclude":true}});
    if mode.teaching {
        body["response_format"] = crate::teaching_protocol::TeachingReply::response_format();
    }
    if mode.tools {
        let tools = crate::references::definitions();
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
    }
    body
}

fn request_once(transport: &Transport<'_>, body: &Value, budget: &Budget) -> Result<Message> {
    let response = transport
        .client
        .post(transport.url)
        .bearer_auth(transport.key)
        .timeout(budget.remaining()?.min(Duration::from_secs(25)))
        .json(body)
        .send()?
        .error_for_status()?;
    let mut bytes = Vec::new();
    response.take(131_073).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 131_072,
        "Coach response exceeded the size limit"
    );
    let response: Completion = serde_json::from_slice(&bytes)?;
    response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message)
        .context("Mercury returned no choices")
}

#[cfg(test)]
#[path = "coach_request_tests.rs"]
mod tests;
