use std::collections::HashMap;

/// A fire-and-forget hook handler. Receives event data by reference.
pub type HookHandler = Box<dyn Fn(&serde_json::Value) + Send + Sync>;

/// A waterfall hook handler. Receives and returns a value that can be transformed.
pub type WaterfallHandler = Box<dyn Fn(serde_json::Value) -> serde_json::Value + Send + Sync>;

/// Registry for hook handlers supporting two dispatch patterns:
///
/// - **Fire-and-forget (`emit`)**: Call all handlers with the event data, ignore return values.
/// - **Waterfall (`emit_waterfall`)**: Pass a value through a chain of handlers, each one
///   receiving the output of the previous handler.
pub struct HookRegistry {
    handlers: HashMap<String, Vec<HookHandler>>,
    waterfall_handlers: HashMap<String, Vec<WaterfallHandler>>,
}

impl HookRegistry {
    /// Create an empty hook registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            waterfall_handlers: HashMap::new(),
        }
    }

    /// Register a fire-and-forget handler for the given event.
    pub fn on(&mut self, event: &str, handler: HookHandler) {
        self.handlers
            .entry(event.to_string())
            .or_default()
            .push(handler);
    }

    /// Register a waterfall handler for the given event.
    pub fn on_waterfall(&mut self, event: &str, handler: WaterfallHandler) {
        self.waterfall_handlers
            .entry(event.to_string())
            .or_default()
            .push(handler);
    }

    /// Fire event to all registered fire-and-forget handlers.
    ///
    /// If no handlers are registered for the event, this is a no-op.
    pub fn emit(&self, event: &str, data: &serde_json::Value) {
        if let Some(handlers) = self.handlers.get(event) {
            for handler in handlers {
                handler(data);
            }
        }
    }

    /// Fire a waterfall event, passing `initial` through the handler chain.
    ///
    /// Each handler receives the output of the previous one. If no handlers are
    /// registered, `initial` is returned unchanged.
    pub fn emit_waterfall(&self, event: &str, initial: serde_json::Value) -> serde_json::Value {
        if let Some(handlers) = self.waterfall_handlers.get(event) {
            let mut value = initial;
            for handler in handlers {
                value = handler(value);
            }
            value
        } else {
            initial
        }
    }

    /// Remove all registered handlers (both fire-and-forget and waterfall).
    pub fn clear(&mut self) {
        self.handlers.clear();
        self.waterfall_handlers.clear();
    }

    /// Check whether any handlers (fire-and-forget or waterfall) are registered
    /// for the given event.
    pub fn has_handlers(&self, event: &str) -> bool {
        let has_regular = self.handlers.get(event).is_some_and(|h| !h.is_empty());
        let has_waterfall = self
            .waterfall_handlers
            .get(event)
            .is_some_and(|h| !h.is_empty());
        has_regular || has_waterfall
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[test]
    fn emit_fires_all_handlers() {
        let mut registry = HookRegistry::new();
        let calls = Arc::new(Mutex::new(Vec::new()));

        let c1 = Arc::clone(&calls);
        registry.on(
            "session:created",
            Box::new(move |data| {
                c1.lock().unwrap().push(format!("h1:{}", data));
            }),
        );

        let c2 = Arc::clone(&calls);
        registry.on(
            "session:created",
            Box::new(move |data| {
                c2.lock().unwrap().push(format!("h2:{}", data));
            }),
        );

        registry.emit("session:created", &json!({"id": 1}));

        let results = calls.lock().unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].starts_with("h1:"));
        assert!(results[1].starts_with("h2:"));
    }

    #[test]
    fn emit_with_no_handlers_does_not_panic() {
        let registry = HookRegistry::new();
        // Should simply be a no-op.
        registry.emit("nonexistent:event", &json!({}));
    }

    #[test]
    fn waterfall_transforms_value_through_chain() {
        let mut registry = HookRegistry::new();

        // First handler doubles the number.
        registry.on_waterfall(
            "render:statusbar",
            Box::new(|val| {
                let n = val.as_i64().unwrap();
                json!(n * 2)
            }),
        );

        // Second handler adds 10.
        registry.on_waterfall(
            "render:statusbar",
            Box::new(|val| {
                let n = val.as_i64().unwrap();
                json!(n + 10)
            }),
        );

        let result = registry.emit_waterfall("render:statusbar", json!(5));
        // 5 * 2 = 10, then 10 + 10 = 20
        assert_eq!(result, json!(20));
    }

    #[test]
    fn waterfall_with_no_handlers_returns_initial() {
        let registry = HookRegistry::new();
        let initial = json!({"title": "hello"});
        let result = registry.emit_waterfall("window:title", initial.clone());
        assert_eq!(result, initial);
    }

    #[test]
    fn clear_removes_all_handlers() {
        let mut registry = HookRegistry::new();

        registry.on("session:created", Box::new(|_| {}));
        registry.on_waterfall("render:statusbar", Box::new(|v| v));

        assert!(registry.has_handlers("session:created"));
        assert!(registry.has_handlers("render:statusbar"));

        registry.clear();

        assert!(!registry.has_handlers("session:created"));
        assert!(!registry.has_handlers("render:statusbar"));
    }

    #[test]
    fn has_handlers_works_correctly() {
        let mut registry = HookRegistry::new();

        assert!(!registry.has_handlers("session:created"));
        assert!(!registry.has_handlers("render:statusbar"));

        registry.on("session:created", Box::new(|_| {}));
        assert!(registry.has_handlers("session:created"));
        assert!(!registry.has_handlers("render:statusbar"));

        registry.on_waterfall("render:statusbar", Box::new(|v| v));
        assert!(registry.has_handlers("render:statusbar"));
    }
}
