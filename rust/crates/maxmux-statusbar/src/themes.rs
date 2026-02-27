// Built-in theme definitions.
//
// Each theme provides bar colors, 8 accent colors, and per-module color pairs.

use crate::types::{ColorPair, ResolvedTheme, ThemeModuleColors};

/// All available built-in theme names.
pub const THEME_NAMES: &[&str] = &[
    "catppuccin-mocha",
    "dracula",
    "nord",
    "tokyo-night",
    "gruvbox",
    "one-dark",
    "solarized",
];

/// The default theme name.
pub const DEFAULT_THEME: &str = "catppuccin-mocha";

/// Resolve a theme by name. Returns the default theme if the name is unknown.
pub fn resolve_theme(name: &str) -> ResolvedTheme {
    match name {
        "catppuccin-mocha" | "catppuccin" => catppuccin_mocha(),
        "dracula" => dracula(),
        "nord" => nord(),
        "tokyo-night" | "tokyonight" => tokyo_night(),
        "gruvbox" => gruvbox(),
        "one-dark" | "onedark" => one_dark(),
        "solarized" => solarized(),
        _ => catppuccin_mocha(),
    }
}

fn catppuccin_mocha() -> ResolvedTheme {
    let dark = "#1e1e2e";
    ResolvedTheme {
        bar: ColorPair::new("#cdd6f4", dark),
        accents: vec![
            "#89b4fa".into(),
            "#a6e3a1".into(),
            "#f9e2af".into(),
            "#cba6f7".into(),
            "#f38ba8".into(),
            "#fab387".into(),
            "#94e2d5".into(),
            "#74c7ec".into(),
        ],
        modules: ThemeModuleColors {
            session: ColorPair::new(dark, "#89b4fa"),
            active_window: ColorPair::new(dark, "#a6e3a1"),
            inactive_window: ColorPair::new("#bac2de", "#313244"),
            git_clean: ColorPair::new(dark, "#a6e3a1"),
            git_dirty: ColorPair::new(dark, "#f9e2af"),
            cpu_low: ColorPair::new(dark, "#a6e3a1"),
            cpu_med: ColorPair::new(dark, "#f9e2af"),
            cpu_high: ColorPair::new(dark, "#f38ba8"),
            battery_high: ColorPair::new(dark, "#a6e3a1"),
            battery_med: ColorPair::new(dark, "#f9e2af"),
            battery_low: ColorPair::new(dark, "#f38ba8"),
            prefix: ColorPair::new(dark, "#f38ba8"),
            prefix_inactive: ColorPair::new("#585b70", "#313244"),
        },
    }
}

fn dracula() -> ResolvedTheme {
    let dark = "#282a36";
    ResolvedTheme {
        bar: ColorPair::new("#f8f8f2", dark),
        accents: vec![
            "#bd93f9".into(),
            "#50fa7b".into(),
            "#f1fa8c".into(),
            "#ff79c6".into(),
            "#ff5555".into(),
            "#ffb86c".into(),
            "#8be9fd".into(),
            "#6272a4".into(),
        ],
        modules: ThemeModuleColors {
            session: ColorPair::new(dark, "#bd93f9"),
            active_window: ColorPair::new(dark, "#50fa7b"),
            inactive_window: ColorPair::new("#f8f8f2", "#44475a"),
            git_clean: ColorPair::new(dark, "#50fa7b"),
            git_dirty: ColorPair::new(dark, "#f1fa8c"),
            cpu_low: ColorPair::new(dark, "#50fa7b"),
            cpu_med: ColorPair::new(dark, "#f1fa8c"),
            cpu_high: ColorPair::new(dark, "#ff5555"),
            battery_high: ColorPair::new(dark, "#50fa7b"),
            battery_med: ColorPair::new(dark, "#f1fa8c"),
            battery_low: ColorPair::new(dark, "#ff5555"),
            prefix: ColorPair::new(dark, "#ff79c6"),
            prefix_inactive: ColorPair::new("#6272a4", "#44475a"),
        },
    }
}

fn nord() -> ResolvedTheme {
    let dark = "#2e3440";
    ResolvedTheme {
        bar: ColorPair::new("#d8dee9", dark),
        accents: vec![
            "#88c0d0".into(),
            "#a3be8c".into(),
            "#ebcb8b".into(),
            "#b48ead".into(),
            "#bf616a".into(),
            "#d08770".into(),
            "#8fbcbb".into(),
            "#81a1c1".into(),
        ],
        modules: ThemeModuleColors {
            session: ColorPair::new(dark, "#88c0d0"),
            active_window: ColorPair::new(dark, "#a3be8c"),
            inactive_window: ColorPair::new("#d8dee9", "#3b4252"),
            git_clean: ColorPair::new(dark, "#a3be8c"),
            git_dirty: ColorPair::new(dark, "#ebcb8b"),
            cpu_low: ColorPair::new(dark, "#a3be8c"),
            cpu_med: ColorPair::new(dark, "#ebcb8b"),
            cpu_high: ColorPair::new(dark, "#bf616a"),
            battery_high: ColorPair::new(dark, "#a3be8c"),
            battery_med: ColorPair::new(dark, "#ebcb8b"),
            battery_low: ColorPair::new(dark, "#bf616a"),
            prefix: ColorPair::new(dark, "#bf616a"),
            prefix_inactive: ColorPair::new("#4c566a", "#3b4252"),
        },
    }
}

fn tokyo_night() -> ResolvedTheme {
    let dark = "#1a1b26";
    ResolvedTheme {
        bar: ColorPair::new("#a9b1d6", dark),
        accents: vec![
            "#7aa2f7".into(),
            "#9ece6a".into(),
            "#e0af68".into(),
            "#bb9af7".into(),
            "#f7768e".into(),
            "#ff9e64".into(),
            "#73daca".into(),
            "#7dcfff".into(),
        ],
        modules: ThemeModuleColors {
            session: ColorPair::new(dark, "#7aa2f7"),
            active_window: ColorPair::new(dark, "#9ece6a"),
            inactive_window: ColorPair::new("#a9b1d6", "#24283b"),
            git_clean: ColorPair::new(dark, "#9ece6a"),
            git_dirty: ColorPair::new(dark, "#e0af68"),
            cpu_low: ColorPair::new(dark, "#9ece6a"),
            cpu_med: ColorPair::new(dark, "#e0af68"),
            cpu_high: ColorPair::new(dark, "#f7768e"),
            battery_high: ColorPair::new(dark, "#9ece6a"),
            battery_med: ColorPair::new(dark, "#e0af68"),
            battery_low: ColorPair::new(dark, "#f7768e"),
            prefix: ColorPair::new(dark, "#f7768e"),
            prefix_inactive: ColorPair::new("#565f89", "#24283b"),
        },
    }
}

fn gruvbox() -> ResolvedTheme {
    let dark = "#282828";
    ResolvedTheme {
        bar: ColorPair::new("#ebdbb2", dark),
        accents: vec![
            "#83a598".into(),
            "#b8bb26".into(),
            "#fabd2f".into(),
            "#d3869b".into(),
            "#fb4934".into(),
            "#fe8019".into(),
            "#8ec07c".into(),
            "#458588".into(),
        ],
        modules: ThemeModuleColors {
            session: ColorPair::new(dark, "#83a598"),
            active_window: ColorPair::new(dark, "#b8bb26"),
            inactive_window: ColorPair::new("#ebdbb2", "#3c3836"),
            git_clean: ColorPair::new(dark, "#b8bb26"),
            git_dirty: ColorPair::new(dark, "#fabd2f"),
            cpu_low: ColorPair::new(dark, "#b8bb26"),
            cpu_med: ColorPair::new(dark, "#fabd2f"),
            cpu_high: ColorPair::new(dark, "#fb4934"),
            battery_high: ColorPair::new(dark, "#b8bb26"),
            battery_med: ColorPair::new(dark, "#fabd2f"),
            battery_low: ColorPair::new(dark, "#fb4934"),
            prefix: ColorPair::new(dark, "#fb4934"),
            prefix_inactive: ColorPair::new("#665c54", "#3c3836"),
        },
    }
}

fn one_dark() -> ResolvedTheme {
    let dark = "#282c34";
    ResolvedTheme {
        bar: ColorPair::new("#abb2bf", dark),
        accents: vec![
            "#61afef".into(),
            "#98c379".into(),
            "#e5c07b".into(),
            "#c678dd".into(),
            "#e06c75".into(),
            "#d19a66".into(),
            "#56b6c2".into(),
            "#528bff".into(),
        ],
        modules: ThemeModuleColors {
            session: ColorPair::new(dark, "#61afef"),
            active_window: ColorPair::new(dark, "#98c379"),
            inactive_window: ColorPair::new("#abb2bf", "#3e4452"),
            git_clean: ColorPair::new(dark, "#98c379"),
            git_dirty: ColorPair::new(dark, "#e5c07b"),
            cpu_low: ColorPair::new(dark, "#98c379"),
            cpu_med: ColorPair::new(dark, "#e5c07b"),
            cpu_high: ColorPair::new(dark, "#e06c75"),
            battery_high: ColorPair::new(dark, "#98c379"),
            battery_med: ColorPair::new(dark, "#e5c07b"),
            battery_low: ColorPair::new(dark, "#e06c75"),
            prefix: ColorPair::new(dark, "#e06c75"),
            prefix_inactive: ColorPair::new("#5c6370", "#3e4452"),
        },
    }
}

fn solarized() -> ResolvedTheme {
    let dark = "#002b36";
    ResolvedTheme {
        bar: ColorPair::new("#839496", dark),
        accents: vec![
            "#268bd2".into(),
            "#859900".into(),
            "#b58900".into(),
            "#6c71c4".into(),
            "#dc322f".into(),
            "#cb4b16".into(),
            "#2aa198".into(),
            "#93a1a1".into(),
        ],
        modules: ThemeModuleColors {
            session: ColorPair::new(dark, "#268bd2"),
            active_window: ColorPair::new(dark, "#859900"),
            inactive_window: ColorPair::new("#839496", "#073642"),
            git_clean: ColorPair::new(dark, "#859900"),
            git_dirty: ColorPair::new(dark, "#b58900"),
            cpu_low: ColorPair::new(dark, "#859900"),
            cpu_med: ColorPair::new(dark, "#b58900"),
            cpu_high: ColorPair::new(dark, "#dc322f"),
            battery_high: ColorPair::new(dark, "#859900"),
            battery_med: ColorPair::new(dark, "#b58900"),
            battery_low: ColorPair::new(dark, "#dc322f"),
            prefix: ColorPair::new(dark, "#dc322f"),
            prefix_inactive: ColorPair::new("#586e75", "#073642"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catppuccin_mocha_bar_colors() {
        let theme = resolve_theme("catppuccin-mocha");
        assert_eq!(theme.bar.bg, "#1e1e2e");
        assert_eq!(theme.bar.fg, "#cdd6f4");
        assert_eq!(theme.accents.len(), 8);
    }

    #[test]
    fn test_dracula_bar_colors() {
        let theme = resolve_theme("dracula");
        assert_eq!(theme.bar.bg, "#282a36");
        assert_eq!(theme.bar.fg, "#f8f8f2");
        assert_eq!(theme.accents.len(), 8);
    }

    #[test]
    fn test_nord_bar_colors() {
        let theme = resolve_theme("nord");
        assert_eq!(theme.bar.bg, "#2e3440");
        assert_eq!(theme.bar.fg, "#d8dee9");
        assert_eq!(theme.accents.len(), 8);
    }

    #[test]
    fn test_tokyo_night_bar_colors() {
        let theme = resolve_theme("tokyo-night");
        assert_eq!(theme.bar.bg, "#1a1b26");
        assert_eq!(theme.bar.fg, "#a9b1d6");
        assert_eq!(theme.accents.len(), 8);
    }

    #[test]
    fn test_gruvbox_bar_colors() {
        let theme = resolve_theme("gruvbox");
        assert_eq!(theme.bar.bg, "#282828");
        assert_eq!(theme.bar.fg, "#ebdbb2");
        assert_eq!(theme.accents.len(), 8);
    }

    #[test]
    fn test_one_dark_bar_colors() {
        let theme = resolve_theme("one-dark");
        assert_eq!(theme.bar.bg, "#282c34");
        assert_eq!(theme.bar.fg, "#abb2bf");
        assert_eq!(theme.accents.len(), 8);
    }

    #[test]
    fn test_solarized_bar_colors() {
        let theme = resolve_theme("solarized");
        assert_eq!(theme.bar.bg, "#002b36");
        assert_eq!(theme.bar.fg, "#839496");
        assert_eq!(theme.accents.len(), 8);
    }

    #[test]
    fn test_unknown_theme_returns_default() {
        let theme = resolve_theme("nonexistent");
        let default = resolve_theme(DEFAULT_THEME);
        assert_eq!(theme.bar, default.bar);
    }

    #[test]
    fn test_alias_catppuccin() {
        let theme = resolve_theme("catppuccin");
        let expected = resolve_theme("catppuccin-mocha");
        assert_eq!(theme.bar, expected.bar);
    }

    #[test]
    fn test_alias_tokyonight() {
        let theme = resolve_theme("tokyonight");
        let expected = resolve_theme("tokyo-night");
        assert_eq!(theme.bar, expected.bar);
    }
}
