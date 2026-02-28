#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::Path;

    use crate::loader::load_config_from_str;
    use crate::schema::{
        BorderStyle, KeybindingValue, LineStyle, MaxmuxConfig, Position, SeparatorStyle,
        SessionListMode, default_keybindings,
    };

    // Convenience: parse a TOML string through the loader pipeline.
    fn parse(toml: &str) -> MaxmuxConfig {
        load_config_from_str(toml, Path::new("<test>")).expect("parse failed")
    }

    // -----------------------------------------------------------------------
    // 1. Default config creation
    // -----------------------------------------------------------------------
    #[test]
    fn default_config_has_expected_values() {
        let cfg = MaxmuxConfig::default();

        assert_eq!(cfg.prefix_key, "C-a");
        assert_eq!(cfg.prefix_timeout, 0);
        assert_eq!(cfg.history_limit, 10_000);
        assert_eq!(cfg.new_pane_cwd, "inherit");
        assert!(cfg.switch_to_new_window);
        assert!(cfg.automatic_rename);
        assert_eq!(cfg.automatic_rename_interval, 2000);
        assert!(cfg.mouse);
        assert!(cfg.show_prefix_help);
        assert!(!cfg.debug);
        assert!(cfg.global_keybindings.is_empty());
        assert!(!cfg.keybindings.is_empty());
        assert!(cfg.plugins.is_empty());
    }

    // -----------------------------------------------------------------------
    // 2. Empty TOML produces defaults (via serde + loader merge)
    // -----------------------------------------------------------------------
    #[test]
    fn empty_toml_produces_defaults() {
        let cfg = parse("");

        assert_eq!(cfg.prefix_key, "C-a");
        assert_eq!(cfg.history_limit, 10_000);
        assert!(cfg.mouse);

        // Keybindings should be the full default set after merge.
        let defaults = default_keybindings();
        assert_eq!(cfg.keybindings.len(), defaults.len());
        assert_eq!(
            cfg.keybindings.get("c"),
            Some(&KeybindingValue::Command("window:create".into()))
        );
    }

    // -----------------------------------------------------------------------
    // 3. TOML parsing with overrides
    // -----------------------------------------------------------------------
    #[test]
    fn toml_overrides_are_applied() {
        let cfg = parse(
            r#"
prefix_key = "C-b"
prefix_timeout = 500
history_limit = 5000
mouse = false
debug = true
automatic_rename = false
"#,
        );

        assert_eq!(cfg.prefix_key, "C-b");
        assert_eq!(cfg.prefix_timeout, 500);
        assert_eq!(cfg.history_limit, 5000);
        assert!(!cfg.mouse);
        assert!(cfg.debug);
        assert!(!cfg.automatic_rename);
        // Non-overridden fields keep defaults.
        assert!(cfg.show_prefix_help);
        assert!(cfg.switch_to_new_window);
    }

    // -----------------------------------------------------------------------
    // 4. Keybinding parsing — string form
    // -----------------------------------------------------------------------
    #[test]
    fn keybinding_string_form() {
        let cfg = parse(
            r#"
[keybindings]
t = "window:create"
"#,
        );

        // User binding is present.
        assert_eq!(
            cfg.keybindings.get("t"),
            Some(&KeybindingValue::Command("window:create".into()))
        );
        // Defaults are still present (merged underneath).
        assert_eq!(
            cfg.keybindings.get("c"),
            Some(&KeybindingValue::Command("window:create".into()))
        );
    }

    // -----------------------------------------------------------------------
    // 5. Keybinding parsing — object form with unless
    // -----------------------------------------------------------------------
    #[test]
    fn keybinding_object_form() {
        let cfg = parse(
            r#"
[keybindings.h]
command = "pane:focus-left"
unless = ["vim", "nvim"]
"#,
        );

        assert_eq!(
            cfg.keybindings.get("h"),
            Some(&KeybindingValue::Conditional {
                command: "pane:focus-left".into(),
                unless: vec!["vim".into(), "nvim".into()],
            })
        );
    }

    // -----------------------------------------------------------------------
    // 6. Invalid values — history_limit out of range
    // -----------------------------------------------------------------------
    #[test]
    fn invalid_history_limit_rejected() {
        let result = load_config_from_str(r#"history_limit = 200000"#, Path::new("<test>"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("history_limit"),
            "error should mention history_limit: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // 7. Invalid values — sidebar_width out of range
    // -----------------------------------------------------------------------
    #[test]
    fn invalid_sidebar_width_rejected() {
        let result = load_config_from_str(
            r#"
[session_list]
sidebar_width = 10
"#,
            Path::new("<test>"),
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("sidebar_width"),
            "error should mention sidebar_width: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // 8. Config file not found → defaults (via load_config)
    // -----------------------------------------------------------------------
    #[test]
    fn load_config_returns_defaults_when_no_file() {
        // We test that find_config_file returns None in a temp dir
        // (no maxmux.toml present).
        let tmp = tempfile::tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let file = crate::loader::find_config_file();
        assert!(file.is_none());

        // Restore CWD.
        std::env::set_current_dir(original_dir).unwrap();
    }

    // -----------------------------------------------------------------------
    // 9. Partial config — only some fields set
    // -----------------------------------------------------------------------
    #[test]
    fn partial_config_fills_defaults() {
        let cfg = parse(
            r#"
prefix_key = "C-b"

[theme.border]
style = "sharp"
"#,
        );

        assert_eq!(cfg.prefix_key, "C-b");
        // Explicitly set field.
        assert_eq!(cfg.theme.border.style, BorderStyle::Sharp);
        // Rest of border config should be defaults.
        assert_eq!(cfg.theme.border.line_style, LineStyle::Solid);
        assert_eq!(cfg.theme.border.fg, "#585b70");
        assert_eq!(cfg.theme.border.active_fg, "#89b4fa");
        // Status bar theme untouched.
        assert_eq!(cfg.theme.status_bar.bg, "#1e1e2e");
    }

    // -----------------------------------------------------------------------
    // 10. Status bar config parsing
    // -----------------------------------------------------------------------
    #[test]
    fn status_bar_config_parsing() {
        let cfg = parse(
            r#"
[status_bar]
enabled = false
position = "top"
theme = "dracula"
icons = false
left = ["session"]
right = ["datetime"]
refresh_interval = 2000
metrics_interval = 10000

[status_bar.separator]
style = "rounded"
left = "|"
right = "|"
"#,
        );

        assert!(!cfg.status_bar.enabled);
        assert_eq!(cfg.status_bar.position, Position::Top);
        assert_eq!(cfg.status_bar.theme, "dracula");
        assert!(!cfg.status_bar.icons);
        assert_eq!(cfg.status_bar.left, vec!["session"]);
        assert_eq!(cfg.status_bar.right, vec!["datetime"]);
        assert_eq!(cfg.status_bar.refresh_interval, 2000);
        assert_eq!(cfg.status_bar.metrics_interval, 10000);
        assert_eq!(cfg.status_bar.separator.style, SeparatorStyle::Rounded);
        assert_eq!(cfg.status_bar.separator.left, Some("|".into()));
        assert_eq!(cfg.status_bar.separator.right, Some("|".into()));
    }

    // -----------------------------------------------------------------------
    // 11. Sessions config parsing
    // -----------------------------------------------------------------------
    #[test]
    fn sessions_config_parsing() {
        let cfg = parse(
            r#"
[sessions]
auto_save = false
auto_save_interval = 60000
auto_restore = false
save_path = "/tmp/maxmux/sessions/"
"#,
        );

        assert!(!cfg.sessions.auto_save);
        assert_eq!(cfg.sessions.auto_save_interval, 60_000);
        assert!(!cfg.sessions.auto_restore);
        assert_eq!(cfg.sessions.save_path, "/tmp/maxmux/sessions/");
    }

    // -----------------------------------------------------------------------
    // 12. Session list config parsing
    // -----------------------------------------------------------------------
    #[test]
    fn session_list_config_parsing() {
        let cfg = parse(
            r#"
[session_list]
mode = "overlay"
sidebar_position = "right"
sidebar_width = 40
"#,
        );

        assert_eq!(cfg.session_list.mode, SessionListMode::Overlay);
        assert_eq!(cfg.session_list.sidebar_position, "right");
        assert_eq!(cfg.session_list.sidebar_width, 40);
    }

    // -----------------------------------------------------------------------
    // 13. Theme config full parsing
    // -----------------------------------------------------------------------
    #[test]
    fn theme_config_full_parsing() {
        let cfg = parse(
            r##"
[theme.status_bar]
bg = "#000000"
fg = "#ffffff"
active = "#ff0000"

[theme.border]
style = "double"
line_style = "dashed"
fg = "#aaaaaa"
active_fg = "#00ff00"
"##,
        );

        assert_eq!(cfg.theme.status_bar.bg, "#000000");
        assert_eq!(cfg.theme.status_bar.fg, "#ffffff");
        assert_eq!(cfg.theme.status_bar.active, "#ff0000");
        assert_eq!(cfg.theme.border.style, BorderStyle::Double);
        assert_eq!(cfg.theme.border.line_style, LineStyle::Dashed);
        assert_eq!(cfg.theme.border.fg, "#aaaaaa");
        assert_eq!(cfg.theme.border.active_fg, "#00ff00");
    }

    // -----------------------------------------------------------------------
    // 14. Keybinding override replaces default
    // -----------------------------------------------------------------------
    #[test]
    fn keybinding_override_replaces_default() {
        let cfg = parse(
            r#"
[keybindings]
c = "session:create"
"#,
        );

        // User override wins.
        assert_eq!(
            cfg.keybindings.get("c"),
            Some(&KeybindingValue::Command("session:create".into()))
        );
        // Other defaults still present.
        assert_eq!(
            cfg.keybindings.get("n"),
            Some(&KeybindingValue::Command("window:next".into()))
        );
    }

    // -----------------------------------------------------------------------
    // 15. Global keybindings parsing
    // -----------------------------------------------------------------------
    #[test]
    fn global_keybindings_parsing() {
        let cfg = parse(
            r#"
[global_keybindings]
"C-h" = "pane:focus-left"
"C-l" = "pane:focus-right"
"#,
        );

        assert_eq!(cfg.global_keybindings.len(), 2);
        assert_eq!(
            cfg.global_keybindings.get("C-h"),
            Some(&KeybindingValue::Command("pane:focus-left".into()))
        );
    }

    // -----------------------------------------------------------------------
    // 16. Loading from a real file on disk
    // -----------------------------------------------------------------------
    #[test]
    fn load_config_from_file() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("maxmux.toml");
        let mut f = std::fs::File::create(&config_path).unwrap();
        writeln!(f, "prefix_key = \"C-b\"").unwrap();
        writeln!(f, "debug = true").unwrap();
        drop(f);

        let cfg = crate::loader::load_config_from_path(&config_path).unwrap();
        assert_eq!(cfg.prefix_key, "C-b");
        assert!(cfg.debug);
        assert!(cfg.mouse); // default preserved
    }

    // -----------------------------------------------------------------------
    // 17. Malformed TOML produces parse error
    // -----------------------------------------------------------------------
    #[test]
    fn malformed_toml_produces_error() {
        let result = load_config_from_str("this is [[ not valid toml", Path::new("<test>"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, crate::loader::ConfigLoadError::Parse { .. }));
    }

    // -----------------------------------------------------------------------
    // 18. Default keybindings completeness
    // -----------------------------------------------------------------------
    #[test]
    fn default_keybindings_has_expected_count() {
        let defaults = default_keybindings();
        // 22 default keybindings as specified in the task.
        assert_eq!(defaults.len(), 22);
        assert!(defaults.contains_key("c"));
        assert!(defaults.contains_key("N"));
        assert!(defaults.contains_key("?"));
    }

    // -----------------------------------------------------------------------
    // 19. StatusBarConfig modules (passthrough TOML values)
    // -----------------------------------------------------------------------
    #[test]
    fn status_bar_modules_passthrough() {
        let cfg = parse(
            r#"
[status_bar.modules.datetime]
format = "%H:%M"
enabled = true
"#,
        );

        let dt = cfg.status_bar.modules.get("datetime").unwrap();
        let table = dt.as_table().unwrap();
        assert_eq!(table.get("format").unwrap().as_str().unwrap(), "%H:%M");
    }

    // -----------------------------------------------------------------------
    // 20. Plugins field accepts arbitrary TOML values
    // -----------------------------------------------------------------------
    #[test]
    fn plugins_accepts_arbitrary_values() {
        let cfg = parse(
            r#"
[[plugins]]
name = "my-plugin"
enabled = true
"#,
        );

        assert_eq!(cfg.plugins.len(), 1);
        let plugin = cfg.plugins[0].as_table().unwrap();
        assert_eq!(plugin.get("name").unwrap().as_str().unwrap(), "my-plugin");
    }

    // -----------------------------------------------------------------------
    // 21. Serialize round-trip
    // -----------------------------------------------------------------------
    #[test]
    fn serialize_roundtrip() {
        let original = MaxmuxConfig::default();
        let toml_str = toml::to_string(&original).expect("serialize");
        let parsed: MaxmuxConfig = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(original, parsed);
    }
}
