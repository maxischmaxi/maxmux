// Command registry module

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Unknown command: {0}")]
    NotFound(String),
    #[error("Command failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct CommandContext {
    pub session_id: String,
    pub window_id: Option<String>,
    pub pane_id: Option<String>,
    pub args: HashMap<String, String>,
}

/// Async command handler type.
pub type CommandHandler = Box<
    dyn Fn(CommandContext) -> Pin<Box<dyn Future<Output = Result<(), CommandError>> + Send>>
        + Send
        + Sync,
>;

pub struct CommandInfo {
    pub id: String,
    pub description: String,
    handler: CommandHandler,
}

pub struct CommandRegistry {
    commands: HashMap<String, CommandInfo>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        id: impl Into<String>,
        description: impl Into<String>,
        handler: CommandHandler,
    ) {
        let id = id.into();
        let info = CommandInfo {
            id: id.clone(),
            description: description.into(),
            handler,
        };
        self.commands.insert(id, info);
    }

    pub async fn execute(&self, id: &str, ctx: CommandContext) -> Result<(), CommandError> {
        let info = self
            .commands
            .get(id)
            .ok_or_else(|| CommandError::NotFound(id.to_string()))?;
        (info.handler)(ctx).await
    }

    pub fn has(&self, id: &str) -> bool {
        self.commands.contains_key(id)
    }

    /// Returns (id, description) pairs for all registered commands.
    pub fn list(&self) -> Vec<(&str, &str)> {
        self.commands
            .values()
            .map(|info| (info.id.as_str(), info.description.as_str()))
            .collect()
    }

    pub fn unregister(&mut self, id: &str) -> bool {
        self.commands.remove(id).is_some()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[tokio::test]
    async fn test_register_and_execute() {
        let mut registry = CommandRegistry::new();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        registry.register(
            "test:cmd",
            "A test command",
            Box::new(move |_ctx| {
                let called = called_clone.clone();
                Box::pin(async move {
                    called.store(true, Ordering::Relaxed);
                    Ok(())
                })
            }),
        );
        let ctx = CommandContext {
            session_id: "s1".into(),
            window_id: None,
            pane_id: None,
            args: HashMap::new(),
        };
        registry.execute("test:cmd", ctx).await.unwrap();
        assert!(called.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_execute_not_found() {
        let registry = CommandRegistry::new();
        let ctx = CommandContext {
            session_id: "s1".into(),
            window_id: None,
            pane_id: None,
            args: HashMap::new(),
        };
        let result = registry.execute("nonexistent", ctx).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CommandError::NotFound(_)));
    }

    #[test]
    fn test_has() {
        let mut registry = CommandRegistry::new();
        registry.register("test:cmd", "desc", Box::new(|_| Box::pin(async { Ok(()) })));
        assert!(registry.has("test:cmd"));
        assert!(!registry.has("other"));
    }

    #[test]
    fn test_list() {
        let mut registry = CommandRegistry::new();
        registry.register("cmd:a", "Alpha", Box::new(|_| Box::pin(async { Ok(()) })));
        registry.register("cmd:b", "Beta", Box::new(|_| Box::pin(async { Ok(()) })));
        let list = registry.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_unregister() {
        let mut registry = CommandRegistry::new();
        registry.register(
            "test:cmd",
            "desc",
            Box::new(|_| Box::pin(async { Ok(()) })),
        );
        assert!(registry.unregister("test:cmd"));
        assert!(!registry.has("test:cmd"));
        assert!(!registry.unregister("test:cmd")); // already removed
    }
}
