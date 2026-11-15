use kroma_module_sdk::host::Event;
use serde_json::Value;

use super::Live;

impl Live {
    pub(crate) fn emit(&self, topic: &str, payload: Value) {
        #[cfg(test)]
        self.sent.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((topic.to_string(), payload.clone()));
        self.host.publish(Event::new(topic, payload));
    }

    #[cfg(test)]
    pub(crate) fn sent_on(&self, topic: &str) -> Vec<Value> {
        self.sent.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().filter(|(t, _)| t == topic).map(|(_, payload)| payload.clone()).collect()
    }
}
