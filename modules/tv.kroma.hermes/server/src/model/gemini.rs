use serde_json::{json, Value};

use super::http::Endpoint;
use super::screen::{self, ScreenReading};
use super::{Allowed, Dialogue, Model, ModelError, ToolCall, ToolDecl, WebAnswer};
use crate::settings::Settings;

const SOURCES_MAX: usize = 3;

#[derive(Clone)]
pub struct Gemini {
    base: String,
    model: String,
    tier: String,
    temperature: Option<f64>,
    key: String,
    timeout_s: u32,
}

impl Gemini {
    pub fn new(settings: &Settings) -> Self {
        Self {
            base: settings.gemini_base.clone(),
            model: settings.gemini_model.trim_start_matches("models/").to_string(),
            tier: settings.gemini_tier.clone(),
            temperature: settings.temperature,
            key: settings.gemini_key.clone(),
            timeout_s: settings.model_timeout_s,
        }
    }

    fn endpoint(&self, purpose: &'static str) -> Endpoint {
        Endpoint {
            purpose,
            timeout_s: self.timeout_s,
            headers: vec![("x-goog-api-key".into(), self.key.clone())],
            secrets: vec![self.key.clone()],
        }
    }

    fn generate_url(&self) -> String {
        format!("{}/v1beta/models/{}:generateContent", self.base, self.model)
    }

    fn generate(&self, purpose: &'static str, body: &Value) -> Result<Value, ModelError> {
        self.endpoint(purpose).post(&self.generate_url(), body)
    }
}

fn text_of(reply: &Value) -> String {
    parts_of(reply).iter().filter_map(|p| p.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("").trim().to_string()
}

fn parts_of(reply: &Value) -> Vec<Value> {
    reply.pointer("/candidates/0/content/parts").and_then(Value::as_array).cloned().unwrap_or_default()
}

fn sources_of(reply: &Value) -> Vec<String> {
    let chunks = reply.pointer("/candidates/0/groundingMetadata/groundingChunks").and_then(Value::as_array);
    let mut out: Vec<String> = Vec::new();
    for title in chunks.into_iter().flatten().filter_map(|c| c.pointer("/web/title").and_then(Value::as_str)) {
        let title: String = title.trim().chars().take(60).collect();
        if !title.is_empty() && !out.contains(&title) {
            out.push(title);
        }
    }
    out.truncate(SOURCES_MAX);
    out
}

impl Model for Gemini {
    fn open(&self, system: &str, first: &str, tools: &[ToolDecl]) -> Box<dyn Dialogue> {
        Box::new(GeminiDialogue {
            api: self.clone(),
            system: system.to_string(),
            declarations: tools.to_vec(),
            contents: vec![json!({ "role": "user", "parts": [{ "text": first }] })],
            pending: Vec::new(),
        })
    }

    fn models(&self) -> Result<Vec<String>, ModelError> {
        let reply = self.endpoint("llm").get(&format!("{}/v1beta/models?pageSize=200", self.base))?;
        Ok(reply
            .get("models")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|m| {
                m.get("supportedGenerationMethods").and_then(Value::as_array).is_none_or(|methods| methods.iter().any(|x| x == "generateContent"))
            })
            .filter_map(|m| m.get("name").and_then(Value::as_str))
            .map(|n| n.trim_start_matches("models/").to_string())
            .collect())
    }

    fn ping(&self) -> Result<String, ModelError> {
        let reply = self.generate("llm", &json!({ "contents": [{ "role": "user", "parts": [{ "text": "Réponds seulement : ok" }] }] }))?;
        Ok(text_of(&reply))
    }

    fn web_search(&self, question: &str) -> Result<WebAnswer, ModelError> {
        let reply = self.generate(
            "llm-search",
            &json!({
                "contents": [{ "role": "user", "parts": [{ "text": question }] }],
                "tools": [{ "googleSearch": {} }],
                "generationConfig": { "temperature": 0.2 },
            }),
        )?;
        Ok(WebAnswer { text: text_of(&reply), sources: sources_of(&reply) })
    }

    fn read_screen(&self, jpeg_base64: &str, question: &str) -> Result<ScreenReading, ModelError> {
        let reply = self.generate(
            "llm",
            &json!({
                "contents": [{ "role": "user", "parts": [
                    { "text": screen::prompt_for(question) },
                    { "inlineData": { "mimeType": "image/jpeg", "data": jpeg_base64 } },
                ] }],
                "generationConfig": { "temperature": 0.1, "responseMimeType": "application/json" },
            }),
        )?;
        Ok(screen::parse_reading(&text_of(&reply)))
    }

    fn can_search(&self) -> bool {
        true
    }
}

struct GeminiDialogue {
    api: Gemini,
    system: String,
    declarations: Vec<ToolDecl>,
    contents: Vec<Value>,
    pending: Vec<ToolCall>,
}

impl GeminiDialogue {
    fn request(&self, allowed: &Allowed) -> Value {
        let mut config = json!({ "mode": "ANY" });
        if let Allowed::Only(names) = allowed {
            config["allowedFunctionNames"] = json!(names);
        }
        let declarations: Vec<Value> = self
            .declarations
            .iter()
            .map(|d| json!({ "name": d.name, "description": d.description, "parameters": d.parameters }))
            .collect();
        let mut body = json!({
            "systemInstruction": { "parts": [{ "text": self.system }] },
            "contents": self.contents,
            "tools": [{ "functionDeclarations": declarations }],
            "toolConfig": { "functionCallingConfig": config },
        });
        if let Some(t) = self.api.temperature {
            body["generationConfig"] = json!({ "temperature": t });
        }
        if !self.api.tier.is_empty() {
            body["serviceTier"] = json!(self.api.tier);
        }
        body
    }
}

fn call_of(part: &Value, index: usize) -> Option<ToolCall> {
    let call = part.get("functionCall")?;
    let name = call.get("name").and_then(Value::as_str)?.to_string();
    let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
    let id = call.get("id").and_then(Value::as_str).map_or_else(|| format!("call-{index}"), str::to_string);
    let malformed = (!args.is_object()).then(|| "the arguments are not a JSON object".to_string());
    Some(ToolCall { id, name, args: if args.is_object() { args } else { json!({}) }, malformed })
}

impl Dialogue for GeminiDialogue {
    fn step(&mut self, allowed: &Allowed) -> Result<Vec<ToolCall>, ModelError> {
        let reply = self.api.generate("llm", &self.request(allowed))?;
        let Some(candidate) = reply.pointer("/candidates/0") else {
            let reason = reply.pointer("/promptFeedback/blockReason").and_then(Value::as_str);
            return match reason {
                Some(reason) => Err(ModelError::Rejected(format!("blocked by the provider: {reason}"))),
                None => Err(ModelError::Malformed("no candidate in the answer".into())),
            };
        };
        let parts = parts_of(&reply);
        let calls: Vec<ToolCall> = parts.iter().enumerate().filter_map(|(i, p)| call_of(p, i)).collect();
        if calls.is_empty() {
            let text = text_of(&reply);
            return Ok(if text.is_empty() { Vec::new() } else { vec![ToolCall::new("reply", json!({ "message": text }))] });
        }
        let mut content = candidate.get("content").cloned().unwrap_or_else(|| json!({ "parts": parts }));
        content["role"] = json!("model");
        self.contents.push(content);
        self.pending = calls.clone();
        Ok(calls)
    }

    fn answer(&mut self, results: &[(ToolCall, Value)]) {
        let parts: Vec<Value> = self
            .pending
            .iter()
            .map(|call| {
                let result = results.iter().find(|(c, _)| c.id == call.id).map_or_else(|| json!("not executed"), |(_, r)| r.clone());
                let mut response = json!({ "name": call.name, "response": { "result": result } });
                if !call.id.starts_with("call-") {
                    response["id"] = json!(call.id);
                }
                json!({ "functionResponse": response })
            })
            .collect();
        self.contents.push(json!({ "role": "user", "parts": parts }));
    }
}
