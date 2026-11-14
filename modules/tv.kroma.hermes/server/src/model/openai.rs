use serde_json::{json, Value};

use super::http::Endpoint;
use super::screen::{self, ScreenReading};
use super::{Allowed, Dialogue, Model, ModelError, ToolCall, ToolDecl, WebAnswer};
use crate::settings::Settings;

#[derive(Clone)]
pub struct OpenAi {
    base: String,
    model: String,
    key: String,
    temperature: Option<f64>,
    timeout_s: u32,
}

impl OpenAi {
    pub fn new(settings: &Settings) -> Self {
        Self {
            base: settings.openai_base.clone(),
            model: settings.openai_model.clone(),
            key: settings.openai_key.clone(),
            temperature: settings.temperature,
            timeout_s: settings.model_timeout_s,
        }
    }

    fn endpoint(&self) -> Endpoint {
        let headers = if self.key.is_empty() { Vec::new() } else { vec![("authorization".to_string(), format!("Bearer {}", self.key))] };
        Endpoint { purpose: "llm-compatible", timeout_s: self.timeout_s, headers, secrets: vec![self.key.clone()] }
    }

    fn complete(&self, body: &Value) -> Result<Value, ModelError> {
        self.endpoint().post(&format!("{}/chat/completions", self.base), body)
    }
}

fn message_of(reply: &Value) -> Result<&Value, ModelError> {
    reply.pointer("/choices/0/message").ok_or_else(|| ModelError::Malformed("no choice in the answer".into()))
}

fn text_of(reply: &Value) -> Result<String, ModelError> {
    Ok(message_of(reply)?.get("content").and_then(Value::as_str).unwrap_or_default().trim().to_string())
}

impl Model for OpenAi {
    fn open(&self, system: &str, first: &str, tools: &[ToolDecl]) -> Box<dyn Dialogue> {
        Box::new(OpenAiDialogue {
            api: self.clone(),
            declarations: tools.to_vec(),
            messages: vec![json!({ "role": "system", "content": system }), json!({ "role": "user", "content": first })],
            pending: Vec::new(),
        })
    }

    fn models(&self) -> Result<Vec<String>, ModelError> {
        let reply = self.endpoint().get(&format!("{}/models", self.base))?;
        Ok(reply.get("data").and_then(Value::as_array).into_iter().flatten().filter_map(|m| m.get("id").and_then(Value::as_str)).map(str::to_string).collect())
    }

    fn ping(&self) -> Result<String, ModelError> {
        let reply = self.complete(&json!({ "model": self.model, "messages": [{ "role": "user", "content": "Réponds seulement : ok" }] }))?;
        text_of(&reply)
    }

    fn web_search(&self, _question: &str) -> Result<WebAnswer, ModelError> {
        Err(ModelError::Rejected("web search needs the Gemini provider".into()))
    }

    fn read_screen(&self, jpeg_base64: &str, question: &str) -> Result<ScreenReading, ModelError> {
        let reply = self.complete(&json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": [
                { "type": "text", "text": screen::prompt_for(question) },
                { "type": "image_url", "image_url": { "url": format!("data:image/jpeg;base64,{jpeg_base64}") } },
            ] }],
            "response_format": { "type": "json_object" },
        }))?;
        Ok(screen::parse_reading(&text_of(&reply)?))
    }

    fn can_search(&self) -> bool {
        false
    }
}

struct OpenAiDialogue {
    api: OpenAi,
    declarations: Vec<ToolDecl>,
    messages: Vec<Value>,
    pending: Vec<ToolCall>,
}

impl OpenAiDialogue {
    fn request(&self, allowed: &Allowed) -> Value {
        let tools: Vec<Value> = self
            .declarations
            .iter()
            .filter(|d| match allowed {
                Allowed::Any => true,
                Allowed::Only(names) => names.contains(&d.name),
            })
            .map(|d| json!({ "type": "function", "function": { "name": d.name, "description": d.description, "parameters": d.parameters } }))
            .collect();
        let mut body = json!({ "model": self.api.model, "messages": self.messages, "tools": tools, "tool_choice": "required" });
        if let Some(t) = self.api.temperature {
            body["temperature"] = json!(t);
        }
        body
    }
}

fn call_of(raw: &Value, index: usize) -> Option<ToolCall> {
    let function = raw.get("function")?;
    let name = function.get("name").and_then(Value::as_str)?.to_string();
    let id = raw.get("id").and_then(Value::as_str).map_or_else(|| format!("call-{index}"), str::to_string);
    let text = function.get("arguments").and_then(Value::as_str).unwrap_or("{}");
    let (args, malformed) = match serde_json::from_str::<Value>(if text.trim().is_empty() { "{}" } else { text }) {
        Ok(v) if v.is_object() => (v, None),
        Ok(_) => (json!({}), Some("the arguments are not a JSON object".to_string())),
        Err(e) => (json!({}), Some(format!("the arguments are not valid JSON ({e})"))),
    };
    Some(ToolCall { id, name, args, malformed })
}

impl Dialogue for OpenAiDialogue {
    fn step(&mut self, allowed: &Allowed) -> Result<Vec<ToolCall>, ModelError> {
        let reply = self.api.complete(&self.request(allowed))?;
        let message = message_of(&reply)?.clone();
        let calls: Vec<ToolCall> = message.get("tool_calls").and_then(Value::as_array).into_iter().flatten().enumerate().filter_map(|(i, c)| call_of(c, i)).collect();
        if calls.is_empty() {
            let text = text_of(&reply)?;
            return Ok(if text.is_empty() { Vec::new() } else { vec![ToolCall::new("reply", json!({ "message": text }))] });
        }
        self.messages.push(message);
        self.pending = calls.clone();
        Ok(calls)
    }

    fn answer(&mut self, results: &[(ToolCall, Value)]) {
        for call in &self.pending {
            let result = results.iter().find(|(c, _)| c.id == call.id).map_or_else(|| json!("not executed"), |(_, r)| r.clone());
            self.messages.push(json!({ "role": "tool", "tool_call_id": call.id, "content": result.to_string() }));
        }
    }
}
