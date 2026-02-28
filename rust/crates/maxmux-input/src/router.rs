use crate::keybindings::KeybindingRegistry;
use crate::keys;
use crate::mouse;

#[derive(Debug, Clone, PartialEq)]
pub enum InputAction {
    Passthrough(Vec<u8>),     // Forward raw bytes to PTY
    Command(String),          // Execute a command
    PrefixActivated,          // Entered prefix mode
    PrefixTimeout,            // Prefix mode timed out
    Mouse(mouse::MouseEvent), // Mouse event to handle
}

pub struct InputRouter {
    prefix_byte: u8,
    #[allow(dead_code)]
    prefix_timeout_ms: u64,
    in_prefix_mode: bool,
    prefix_bindings: KeybindingRegistry,
    global_bindings: KeybindingRegistry,
}

impl InputRouter {
    pub fn new(prefix_key: &str, timeout_ms: u64) -> Self {
        let prefix_byte = keys::parse_prefix_key(prefix_key).unwrap_or(0x01); // default C-a
        InputRouter {
            prefix_byte,
            prefix_timeout_ms: timeout_ms,
            in_prefix_mode: false,
            prefix_bindings: KeybindingRegistry::new(),
            global_bindings: KeybindingRegistry::new(),
        }
    }

    /// Process raw input bytes and return actions.
    ///
    /// Algorithm:
    /// 1. Try to parse mouse event first (ESC[< prefix)
    ///    If found, return Mouse action
    /// 2. If in prefix mode:
    ///    Parse key, look up in prefix_bindings
    ///    If found: return Command, exit prefix mode
    ///    If not found: passthrough, exit prefix mode
    /// 3. If not in prefix mode:
    ///    Check if byte == prefix_byte -> enter prefix mode
    ///    Parse key, check global_bindings
    ///    If found: return Command
    ///    Else: return Passthrough
    pub fn handle_input(&mut self, data: &[u8], current_process: Option<&str>) -> Vec<InputAction> {
        let mut actions = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            // 1. Try to parse mouse event first
            if let Some((event, consumed)) = mouse::parse_sgr_mouse(&data[offset..]) {
                actions.push(InputAction::Mouse(event));
                offset += consumed;
                continue;
            }

            // 2. If in prefix mode
            if self.in_prefix_mode {
                let (key, consumed) = keys::parse_key(&data[offset..]);
                if consumed == 0 {
                    break;
                }
                let key_str = keys::key_name(&key);

                if let Some(cmd) = self.prefix_bindings.resolve(&key_str, current_process) {
                    actions.push(InputAction::Command(cmd.to_string()));
                } else {
                    // Not a bound prefix key: pass through the raw bytes
                    actions.push(InputAction::Passthrough(
                        data[offset..offset + consumed].to_vec(),
                    ));
                }
                self.in_prefix_mode = false;
                offset += consumed;
                continue;
            }

            // 3. Not in prefix mode
            // Check for prefix key activation
            if data[offset] == self.prefix_byte {
                self.in_prefix_mode = true;
                actions.push(InputAction::PrefixActivated);
                offset += 1;
                continue;
            }

            // Parse key and check global bindings
            let (key, consumed) = keys::parse_key(&data[offset..]);
            if consumed == 0 {
                break;
            }
            let key_str = keys::key_name(&key);

            if let Some(cmd) = self.global_bindings.resolve(&key_str, current_process) {
                actions.push(InputAction::Command(cmd.to_string()));
            } else {
                actions.push(InputAction::Passthrough(
                    data[offset..offset + consumed].to_vec(),
                ));
            }
            offset += consumed;
        }

        actions
    }

    /// Get immutable reference to prefix-mode keybindings.
    pub fn prefix_bindings(&self) -> &KeybindingRegistry {
        &self.prefix_bindings
    }

    /// Get mutable reference to prefix-mode keybindings.
    pub fn prefix_bindings_mut(&mut self) -> &mut KeybindingRegistry {
        &mut self.prefix_bindings
    }

    /// Get mutable reference to global keybindings.
    pub fn global_bindings_mut(&mut self) -> &mut KeybindingRegistry {
        &mut self.global_bindings
    }

    /// Check if the router is currently in prefix mode.
    pub fn is_in_prefix_mode(&self) -> bool {
        self.in_prefix_mode
    }

    /// Cancel prefix mode without dispatching a command.
    pub fn cancel_prefix(&mut self) {
        self.in_prefix_mode = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_mode_activation() {
        let mut router = InputRouter::new("C-a", 0);
        let actions = router.handle_input(&[0x01], None); // Ctrl+a
        assert!(matches!(&actions[0], InputAction::PrefixActivated));
        assert!(router.is_in_prefix_mode());
    }

    #[test]
    fn test_prefix_command_dispatch() {
        let mut router = InputRouter::new("C-a", 0);
        router
            .prefix_bindings_mut()
            .bind("c".into(), "window:create".into(), vec![]);
        router.handle_input(&[0x01], None); // Enter prefix
        let actions = router.handle_input(b"c", None);
        assert!(matches!(&actions[0], InputAction::Command(cmd) if cmd == "window:create"));
        assert!(!router.is_in_prefix_mode());
    }

    #[test]
    fn test_global_binding() {
        let mut router = InputRouter::new("C-a", 0);
        router
            .global_bindings_mut()
            .bind("C-h".into(), "pane:focus-left".into(), vec![]);
        let actions = router.handle_input(&[0x08], None); // Ctrl+h = 0x08
        assert!(matches!(&actions[0], InputAction::Command(cmd) if cmd == "pane:focus-left"));
    }

    #[test]
    fn test_passthrough() {
        let mut router = InputRouter::new("C-a", 0);
        let actions = router.handle_input(b"hello", None);
        // Should be passthrough for each character
        assert!(
            actions
                .iter()
                .all(|a| matches!(a, InputAction::Passthrough(_)))
        );
    }

    #[test]
    fn test_mouse_event() {
        let mut router = InputRouter::new("C-a", 0);
        let actions = router.handle_input(b"\x1b[<0;10;5M", None);
        assert!(matches!(&actions[0], InputAction::Mouse(ev) if ev.x == 9 && ev.y == 4));
    }

    #[test]
    fn test_cancel_prefix() {
        let mut router = InputRouter::new("C-a", 0);
        router.handle_input(&[0x01], None); // Enter prefix
        assert!(router.is_in_prefix_mode());
        router.cancel_prefix();
        assert!(!router.is_in_prefix_mode());
    }

    #[test]
    fn test_prefix_unbound_key_passthrough() {
        let mut router = InputRouter::new("C-a", 0);
        router.handle_input(&[0x01], None); // Enter prefix
        let actions = router.handle_input(b"z", None); // 'z' is not bound
        assert!(matches!(&actions[0], InputAction::Passthrough(_)));
        assert!(!router.is_in_prefix_mode());
    }

    #[test]
    fn test_multiple_keys_in_one_input() {
        let mut router = InputRouter::new("C-a", 0);
        let actions = router.handle_input(b"ab", None);
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], InputAction::Passthrough(d) if d == b"a"));
        assert!(matches!(&actions[1], InputAction::Passthrough(d) if d == b"b"));
    }

    #[test]
    fn test_prefix_then_command_in_sequence() {
        let mut router = InputRouter::new("C-a", 0);
        router
            .prefix_bindings_mut()
            .bind("c".into(), "window:create".into(), vec![]);
        // Send prefix key followed by 'c' in a single input
        let actions = router.handle_input(&[0x01, b'c'], None);
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], InputAction::PrefixActivated));
        assert!(matches!(&actions[1], InputAction::Command(cmd) if cmd == "window:create"));
        assert!(!router.is_in_prefix_mode());
    }

    #[test]
    fn test_global_binding_unless() {
        let mut router = InputRouter::new("C-a", 0);
        router.global_bindings_mut().bind(
            "C-h".into(),
            "pane:focus-left".into(),
            vec!["vim".into()],
        );
        // Should not trigger when vim is running
        let actions = router.handle_input(&[0x08], Some("vim"));
        assert!(matches!(&actions[0], InputAction::Passthrough(_)));
        // Should trigger when bash is running
        let actions = router.handle_input(&[0x08], Some("bash"));
        assert!(matches!(&actions[0], InputAction::Command(cmd) if cmd == "pane:focus-left"));
    }
}
