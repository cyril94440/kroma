#[cfg(not(target_family = "wasm"))]
pub mod fake;
#[cfg(not(target_family = "wasm"))]
mod gemini;
#[cfg(not(target_family = "wasm"))]
mod openai;
mod screen;
