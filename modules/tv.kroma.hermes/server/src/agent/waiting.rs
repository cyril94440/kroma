use std::sync::Arc;

use serde_json::Value;

use super::deps::Deps;
use crate::model::{Model, ToolCall};
use crate::tools::Follow;

pub struct Waiting {
    follow: Follow,
    slot: usize,
    results: Vec<(ToolCall, Value)>,
}

impl Waiting {
    pub fn new(follow: Follow, slot: usize, results: Vec<(ToolCall, Value)>) -> Self {
        Self { follow, slot, results }
    }

    pub fn step(&mut self, deps: &Deps, model: &Arc<dyn Model>) -> Option<Vec<(ToolCall, Value)>> {
        let out = self.follow.step(deps, model)?;
        self.results[self.slot].1 = out;
        Some(std::mem::take(&mut self.results))
    }
}
