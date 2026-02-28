use std::collections::HashMap;

/// Resolves format variables in template strings.
///
/// Format variables use the syntax `#{variable_name}`, similar to tmux.
/// For example: `#{session_name}`, `#{window_index}`, `#{pane_id}`.
pub struct FormatResolver {
    vars: HashMap<String, String>,
}

impl FormatResolver {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    /// Set a format variable.
    pub fn set(&mut self, key: &str, value: &str) {
        self.vars.insert(key.to_string(), value.to_string());
    }

    /// Resolve format variables in a template string.
    ///
    /// Replaces all occurrences of `#{key}` with the corresponding value.
    /// Unknown variables are left as-is in the output.
    pub fn resolve(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, value) in &self.vars {
            result = result.replace(&format!("#{{{}}}", key), value);
        }
        result
    }
}

impl Default for FormatResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a FormatResolver populated with the current session state.
pub fn build_resolver(
    session_name: &str,
    session_id: &str,
    window_index: usize,
    window_name: &str,
    pane_id: &str,
    pane_index: usize,
) -> FormatResolver {
    let mut r = FormatResolver::new();
    r.set("session_name", session_name);
    r.set("session_id", session_id);
    r.set("window_index", &window_index.to_string());
    r.set("window_name", window_name);
    r.set("pane_id", pane_id);
    r.set("pane_index", &pane_index.to_string());
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_single_variable() {
        let mut r = FormatResolver::new();
        r.set("session_name", "main");
        assert_eq!(r.resolve("Session: #{session_name}"), "Session: main");
    }

    #[test]
    fn test_resolve_unknown_variable_kept() {
        let r = FormatResolver::new();
        let input = "#{unknown_var}";
        assert_eq!(r.resolve(input), "#{unknown_var}");
    }

    #[test]
    fn test_resolve_empty_template() {
        let r = FormatResolver::new();
        assert_eq!(r.resolve(""), "");
    }

    #[test]
    fn test_resolve_multiple_variables() {
        let mut r = FormatResolver::new();
        r.set("session_name", "dev");
        r.set("window_index", "2");
        r.set("pane_id", "p3");
        let result = r.resolve("#{session_name}:#{window_index}.#{pane_id}");
        assert_eq!(result, "dev:2.p3");
    }

    #[test]
    fn test_resolve_repeated_variable() {
        let mut r = FormatResolver::new();
        r.set("name", "foo");
        assert_eq!(r.resolve("#{name}-#{name}"), "foo-foo");
    }

    #[test]
    fn test_resolve_no_variables_in_plain_text() {
        let r = FormatResolver::new();
        assert_eq!(r.resolve("plain text"), "plain text");
    }

    #[test]
    fn test_build_resolver() {
        let r = build_resolver("main", "s1", 0, "shell", "p1", 0);
        let result = r.resolve("#{session_name}:#{window_index}.#{pane_id}");
        assert_eq!(result, "main:0.p1");
    }

    #[test]
    fn test_resolve_mixed_known_and_unknown() {
        let mut r = FormatResolver::new();
        r.set("host", "server1");
        let result = r.resolve("#{host} running #{unknown}");
        assert_eq!(result, "server1 running #{unknown}");
    }
}
