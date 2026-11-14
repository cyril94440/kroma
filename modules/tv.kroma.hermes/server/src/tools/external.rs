use serde_json::{json, Value};

use super::{failure, ToolEnv};
use crate::blackbox::clip;

const RESULT_MAX: usize = 12_000;

pub fn call(env: &mut ToolEnv, module_id: &str, name: &str, args: &Value) -> Value {
    let result = env.deps.contributors.call(module_id, name, args, &env.message.user_id, &env.message.id);
    env.deps.audit("sources", "external_tool", json!({ "module": module_id, "tool": name, "args": args, "result": format!("{result:?}") }));
    match result {
        Ok(value) if value.to_string().len() > RESULT_MAX => json!({ "tronque": true, "debut": clip(&value.to_string(), RESULT_MAX) }),
        Ok(value) => value,
        Err(e) => failure(&format!("outil de {module_id} indisponible : {e:#}")),
    }
}
