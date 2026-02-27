use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Keybinding {
    pub command_id: String,
    pub unless: Vec<String>,
}

pub struct KeybindingRegistry {
    bindings: HashMap<String, Keybinding>,
}

impl KeybindingRegistry {
    pub fn new() -> Self {
        KeybindingRegistry {
            bindings: HashMap::new(),
        }
    }

    /// Bind a key to a command with optional "unless" conditions.
    /// The "unless" list contains process names that should block this binding.
    pub fn bind(&mut self, key: String, command_id: String, unless: Vec<String>) {
        self.bindings.insert(
            key,
            Keybinding {
                command_id,
                unless,
            },
        );
    }

    /// Resolve a key to its command, respecting "unless" conditions.
    /// Returns None if the key is not bound, or if the current process
    /// matches one of the "unless" conditions.
    pub fn resolve(&self, key: &str, current_process: Option<&str>) -> Option<&str> {
        let binding = self.bindings.get(key)?;

        // Check unless conditions
        if let Some(proc) = current_process {
            if binding.unless.iter().any(|u| u == proc) {
                return None;
            }
        }

        Some(&binding.command_id)
    }

    /// Get all bindings.
    pub fn all(&self) -> &HashMap<String, Keybinding> {
        &self.bindings
    }

    /// Remove a keybinding. Returns true if it existed.
    pub fn unbind(&mut self, key: &str) -> bool {
        self.bindings.remove(key).is_some()
    }

    /// Remove all keybindings.
    pub fn clear(&mut self) {
        self.bindings.clear();
    }
}

impl Default for KeybindingRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bind_and_resolve() {
        let mut reg = KeybindingRegistry::new();
        reg.bind("c".into(), "window:create".into(), vec![]);
        assert_eq!(reg.resolve("c", None), Some("window:create"));
    }

    #[test]
    fn test_unless_blocks() {
        let mut reg = KeybindingRegistry::new();
        reg.bind(
            "C-l".into(),
            "pane:focus-right".into(),
            vec!["vim".into()],
        );
        assert_eq!(reg.resolve("C-l", Some("vim")), None);
        assert_eq!(
            reg.resolve("C-l", Some("bash")),
            Some("pane:focus-right")
        );
    }

    #[test]
    fn test_resolve_missing() {
        let reg = KeybindingRegistry::new();
        assert_eq!(reg.resolve("x", None), None);
    }

    #[test]
    fn test_unbind() {
        let mut reg = KeybindingRegistry::new();
        reg.bind("c".into(), "window:create".into(), vec![]);
        assert!(reg.unbind("c"));
        assert!(!reg.unbind("c"));
        assert_eq!(reg.resolve("c", None), None);
    }

    #[test]
    fn test_clear() {
        let mut reg = KeybindingRegistry::new();
        reg.bind("c".into(), "window:create".into(), vec![]);
        reg.bind("d".into(), "window:destroy".into(), vec![]);
        reg.clear();
        assert_eq!(reg.all().len(), 0);
    }

    #[test]
    fn test_all() {
        let mut reg = KeybindingRegistry::new();
        reg.bind("a".into(), "cmd_a".into(), vec![]);
        reg.bind("b".into(), "cmd_b".into(), vec![]);
        assert_eq!(reg.all().len(), 2);
    }

    #[test]
    fn test_unless_no_process() {
        let mut reg = KeybindingRegistry::new();
        reg.bind(
            "C-l".into(),
            "pane:focus-right".into(),
            vec!["vim".into()],
        );
        // No current process, should still resolve
        assert_eq!(reg.resolve("C-l", None), Some("pane:focus-right"));
    }

    #[test]
    fn test_overwrite_binding() {
        let mut reg = KeybindingRegistry::new();
        reg.bind("c".into(), "old_cmd".into(), vec![]);
        reg.bind("c".into(), "new_cmd".into(), vec![]);
        assert_eq!(reg.resolve("c", None), Some("new_cmd"));
    }
}
