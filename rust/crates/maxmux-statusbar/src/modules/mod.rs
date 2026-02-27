// Status bar modules.
//
// Each module implements the StatusBarModule trait and produces Segment values
// that the renderer composes into the final status bar line.

pub mod battery;
pub mod cpu;
pub mod cwd;
pub mod datetime;
pub mod git;
pub mod hostname;
pub mod network;
pub mod notes;
pub mod pane_info;
pub mod prefix;
pub mod ram;
pub mod session;
pub mod user;
pub mod windows;

use crate::types::{ColorPair, ResolvedTheme, Segment};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Context types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: String,
    pub name: String,
    pub index: usize,
    pub pane_count: usize,
    pub is_active: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GitInfo {
    pub branch: String,
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Default)]
pub struct CpuInfo {
    pub usage: f64, // 0-100
}

#[derive(Debug, Clone, Default)]
pub struct MemoryInfo {
    pub used_mb: f64,
    pub total_mb: f64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Default)]
pub struct BatteryInfo {
    pub level: u8,     // 0-100
    pub charging: bool,
    pub present: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkInfo {
    pub interface: String,
    pub ip: String,
}

#[derive(Debug, Clone, Default)]
pub struct SystemMetrics {
    pub hostname: Option<String>,
    pub username: Option<String>,
    pub cwd: Option<String>,
    pub git: Option<GitInfo>,
    pub cpu: Option<CpuInfo>,
    pub memory: Option<MemoryInfo>,
    pub battery: Option<BatteryInfo>,
    pub network: Option<NetworkInfo>,
    pub pane_count: usize,
    pub pane_title: Option<String>,
    pub notes_count: usize,
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
}

pub struct ModuleContext {
    pub session: SessionInfo,
    pub windows: Vec<WindowInfo>,
    pub metrics: SystemMetrics,
    pub prefix_active: bool,
    pub cols: u16,
    pub rows: u16,
    pub colors: ColorPair,
    pub theme_colors: ResolvedTheme,
    pub module_config: HashMap<String, toml::Value>,
    pub icons: bool,
}

// ---------------------------------------------------------------------------
// Module trait
// ---------------------------------------------------------------------------

pub trait StatusBarModule {
    fn id(&self) -> &str;
    fn render(&self, ctx: &ModuleContext) -> Vec<Segment>;
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub fn build_module_registry() -> HashMap<String, Box<dyn StatusBarModule>> {
    let mut map: HashMap<String, Box<dyn StatusBarModule>> = HashMap::new();

    let modules: Vec<Box<dyn StatusBarModule>> = vec![
        Box::new(session::SessionModule),
        Box::new(windows::WindowsModule),
        Box::new(datetime::DatetimeModule),
        Box::new(hostname::HostnameModule),
        Box::new(user::UserModule),
        Box::new(cwd::CwdModule),
        Box::new(git::GitModule),
        Box::new(cpu::CpuModule),
        Box::new(ram::RamModule),
        Box::new(battery::BatteryModule),
        Box::new(network::NetworkModule),
        Box::new(prefix::PrefixModule),
        Box::new(pane_info::PaneInfoModule),
        Box::new(notes::NotesModule),
    ];

    for m in modules {
        map.insert(m.id().to_string(), m);
    }

    map
}

pub fn render_module(
    id: &str,
    ctx: &ModuleContext,
    registry: &HashMap<String, Box<dyn StatusBarModule>>,
) -> Vec<Segment> {
    registry.get(id).map(|m| m.render(ctx)).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Helper used by many modules
// ---------------------------------------------------------------------------

/// Get a string config value from the module_config map.
pub(crate) fn config_str<'a>(ctx: &'a ModuleContext, key: &str, default: &'a str) -> String {
    ctx.module_config
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}

/// Get a bool config value from the module_config map.
pub(crate) fn config_bool(ctx: &ModuleContext, key: &str, default: bool) -> bool {
    ctx.module_config
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

/// Get an integer config value from the module_config map.
pub(crate) fn config_int(ctx: &ModuleContext, key: &str, default: i64) -> i64 {
    ctx.module_config
        .get(key)
        .and_then(|v| v.as_integer())
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::resolve_theme;

    /// Create a default test context with sensible defaults.
    fn test_ctx() -> ModuleContext {
        let theme = resolve_theme("catppuccin-mocha");
        ModuleContext {
            session: SessionInfo {
                id: "sess-1".into(),
                name: "dev".into(),
            },
            windows: vec![],
            metrics: SystemMetrics::default(),
            prefix_active: false,
            cols: 120,
            rows: 40,
            colors: ColorPair::new("#1e1e2e", "#89b4fa"),
            theme_colors: theme,
            module_config: HashMap::new(),
            icons: true,
        }
    }

    // -----------------------------------------------------------------------
    // 1. Session module
    // -----------------------------------------------------------------------

    #[test]
    fn test_session_renders_name_with_icon() {
        let ctx = test_ctx();
        let m = session::SessionModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("dev"));
        assert!(segs[0].text.contains('\u{f0135}')); // icon
        assert!(segs[0].bold);
    }

    #[test]
    fn test_session_without_icons() {
        let mut ctx = test_ctx();
        ctx.icons = false;
        let m = session::SessionModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("dev"));
        assert!(!segs[0].text.contains('\u{f0135}'));
        assert!(segs[0].bold);
    }

    // -----------------------------------------------------------------------
    // 2. Windows module
    // -----------------------------------------------------------------------

    #[test]
    fn test_windows_active_and_inactive() {
        let mut ctx = test_ctx();
        ctx.windows = vec![
            WindowInfo {
                id: "w1".into(),
                name: "code".into(),
                index: 0,
                pane_count: 1,
                is_active: true,
            },
            WindowInfo {
                id: "w2".into(),
                name: "logs".into(),
                index: 1,
                pane_count: 1,
                is_active: false,
            },
        ];
        let m = windows::WindowsModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 2);
        // Active window: bold, has *.
        assert!(segs[0].text.contains("0:code*"));
        assert!(segs[0].bold);
        // Inactive window: not bold, has -.
        assert!(segs[1].text.contains("1:logs-"));
        assert!(!segs[1].bold);
    }

    #[test]
    fn test_windows_circled_numbering() {
        let mut ctx = test_ctx();
        ctx.module_config
            .insert("numbering".into(), toml::Value::String("circled".into()));
        ctx.windows = vec![WindowInfo {
            id: "w1".into(),
            name: "main".into(),
            index: 2,
            pane_count: 1,
            is_active: true,
        }];
        let m = windows::WindowsModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        // Index 2 -> circled ③.
        assert!(segs[0].text.contains('\u{2462}'));
    }

    #[test]
    fn test_windows_number_numbering() {
        let mut ctx = test_ctx();
        ctx.module_config
            .insert("numbering".into(), toml::Value::String("number".into()));
        ctx.windows = vec![WindowInfo {
            id: "w1".into(),
            name: "main".into(),
            index: 0,
            pane_count: 1,
            is_active: true,
        }];
        let m = windows::WindowsModule;
        let segs = m.render(&ctx);
        // Index 0 with "number" -> displays as "1".
        assert!(segs[0].text.contains("1:main*"));
    }

    #[test]
    fn test_windows_bracketed_style() {
        let mut ctx = test_ctx();
        ctx.module_config
            .insert("style".into(), toml::Value::String("bracketed".into()));
        ctx.windows = vec![WindowInfo {
            id: "w1".into(),
            name: "edit".into(),
            index: 3,
            pane_count: 1,
            is_active: false,
        }];
        let m = windows::WindowsModule;
        let segs = m.render(&ctx);
        assert!(segs[0].text.contains("[3] edit"));
    }

    // -----------------------------------------------------------------------
    // 3. Datetime module
    // -----------------------------------------------------------------------

    #[test]
    fn test_datetime_default_format() {
        let ctx = test_ctx();
        let m = datetime::DatetimeModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        // Should contain HH:mm pattern (digits:digits).
        let text = &segs[0].text;
        // Just check it has a colon between digits.
        assert!(text.contains(':'));
    }

    #[test]
    fn test_datetime_custom_format() {
        let mut ctx = test_ctx();
        ctx.module_config
            .insert("format".into(), toml::Value::String("YYYY-MM-DD".into()));
        let m = datetime::DatetimeModule;
        let segs = m.render(&ctx);
        let text = &segs[0].text;
        // Should contain a date pattern like 2026-02-28.
        assert!(text.contains('-'));
        // Should NOT contain a colon (no time part).
        // (unless the date itself happens to contain one, which it won't).
    }

    // -----------------------------------------------------------------------
    // 4. Git module
    // -----------------------------------------------------------------------

    #[test]
    fn test_git_dirty_branch() {
        let mut ctx = test_ctx();
        ctx.metrics.git = Some(GitInfo {
            branch: "feature".into(),
            dirty: true,
            ahead: 0,
            behind: 0,
        });
        let m = git::GitModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("feature"));
        assert!(segs[0].text.contains('*'));
        assert!(segs[0].bold);
        // Uses git_dirty colors.
        assert_eq!(segs[0].bg, ctx.theme_colors.modules.git_dirty.bg);
    }

    #[test]
    fn test_git_clean_branch() {
        let mut ctx = test_ctx();
        ctx.metrics.git = Some(GitInfo {
            branch: "main".into(),
            dirty: false,
            ahead: 0,
            behind: 0,
        });
        let m = git::GitModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("main"));
        assert!(!segs[0].text.contains('*'));
        assert!(!segs[0].bold);
        assert_eq!(segs[0].bg, ctx.theme_colors.modules.git_clean.bg);
    }

    #[test]
    fn test_git_ahead_behind() {
        let mut ctx = test_ctx();
        ctx.metrics.git = Some(GitInfo {
            branch: "dev".into(),
            dirty: false,
            ahead: 3,
            behind: 1,
        });
        let m = git::GitModule;
        let segs = m.render(&ctx);
        assert!(segs[0].text.contains("\u{2191}3")); // ↑3
        assert!(segs[0].text.contains("\u{2193}1")); // ↓1
    }

    #[test]
    fn test_git_empty_when_no_info() {
        let ctx = test_ctx();
        let m = git::GitModule;
        let segs = m.render(&ctx);
        assert!(segs.is_empty());
    }

    // -----------------------------------------------------------------------
    // 5. CPU module
    // -----------------------------------------------------------------------

    #[test]
    fn test_cpu_low_threshold() {
        let mut ctx = test_ctx();
        ctx.metrics.cpu = Some(CpuInfo { usage: 25.0 });
        let m = cpu::CpuModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("25%"));
        // Should use cpu_low colors (< 50).
        assert_eq!(segs[0].bg, ctx.theme_colors.modules.cpu_low.bg);
    }

    #[test]
    fn test_cpu_med_threshold() {
        let mut ctx = test_ctx();
        ctx.metrics.cpu = Some(CpuInfo { usage: 65.0 });
        let m = cpu::CpuModule;
        let segs = m.render(&ctx);
        assert_eq!(segs[0].bg, ctx.theme_colors.modules.cpu_med.bg);
    }

    #[test]
    fn test_cpu_high_threshold() {
        let mut ctx = test_ctx();
        ctx.metrics.cpu = Some(CpuInfo { usage: 95.0 });
        let m = cpu::CpuModule;
        let segs = m.render(&ctx);
        assert_eq!(segs[0].bg, ctx.theme_colors.modules.cpu_high.bg);
    }

    #[test]
    fn test_cpu_with_bar() {
        let mut ctx = test_ctx();
        ctx.metrics.cpu = Some(CpuInfo { usage: 60.0 });
        ctx.module_config
            .insert("showBar".into(), toml::Value::Boolean(true));
        let m = cpu::CpuModule;
        let segs = m.render(&ctx);
        // Should contain bar characters.
        assert!(
            segs[0].text.contains('\u{2593}') || segs[0].text.contains('\u{2591}')
        );
    }

    #[test]
    fn test_cpu_empty_when_no_info() {
        let ctx = test_ctx();
        let m = cpu::CpuModule;
        let segs = m.render(&ctx);
        assert!(segs.is_empty());
    }

    // -----------------------------------------------------------------------
    // 6. Battery module
    // -----------------------------------------------------------------------

    #[test]
    fn test_battery_high_level() {
        let mut ctx = test_ctx();
        ctx.metrics.battery = Some(BatteryInfo {
            level: 95,
            charging: false,
            present: true,
        });
        let m = battery::BatteryModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("95%"));
        assert!(segs[0].text.contains('\u{f0079}')); // 󰁹 full icon
        assert_eq!(segs[0].bg, ctx.theme_colors.modules.battery_high.bg);
    }

    #[test]
    fn test_battery_low_level() {
        let mut ctx = test_ctx();
        ctx.metrics.battery = Some(BatteryInfo {
            level: 5,
            charging: false,
            present: true,
        });
        let m = battery::BatteryModule;
        let segs = m.render(&ctx);
        assert!(segs[0].text.contains("5%"));
        assert!(segs[0].text.contains('\u{f0083}')); // 󰂃 empty icon
        assert_eq!(segs[0].bg, ctx.theme_colors.modules.battery_low.bg);
    }

    #[test]
    fn test_battery_charging() {
        let mut ctx = test_ctx();
        ctx.metrics.battery = Some(BatteryInfo {
            level: 50,
            charging: true,
            present: true,
        });
        let m = battery::BatteryModule;
        let segs = m.render(&ctx);
        assert!(segs[0].text.contains('\u{26a1}')); // ⚡
        assert!(segs[0].text.contains('\u{f0084}')); // 󰂄 charging icon
    }

    #[test]
    fn test_battery_not_present() {
        let mut ctx = test_ctx();
        ctx.metrics.battery = Some(BatteryInfo {
            level: 0,
            charging: false,
            present: false,
        });
        let m = battery::BatteryModule;
        let segs = m.render(&ctx);
        assert!(segs.is_empty());
    }

    #[test]
    fn test_battery_empty_when_no_info() {
        let ctx = test_ctx();
        let m = battery::BatteryModule;
        let segs = m.render(&ctx);
        assert!(segs.is_empty());
    }

    // -----------------------------------------------------------------------
    // 7. Prefix module
    // -----------------------------------------------------------------------

    #[test]
    fn test_prefix_active() {
        let mut ctx = test_ctx();
        ctx.prefix_active = true;
        let m = prefix::PrefixModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("PREFIX"));
        assert!(segs[0].bold);
        assert_eq!(segs[0].bg, ctx.theme_colors.modules.prefix.bg);
    }

    #[test]
    fn test_prefix_inactive() {
        let mut ctx = test_ctx();
        ctx.prefix_active = false;
        let m = prefix::PrefixModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("WAIT"));
        assert!(!segs[0].bold);
        assert_eq!(segs[0].bg, ctx.theme_colors.modules.prefix_inactive.bg);
    }

    // -----------------------------------------------------------------------
    // 8. CWD module
    // -----------------------------------------------------------------------

    #[test]
    fn test_cwd_short_path() {
        let mut ctx = test_ctx();
        ctx.metrics.cwd = Some("~/src".into());
        let m = cwd::CwdModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("~/src"));
    }

    #[test]
    fn test_cwd_path_truncation() {
        // Test the truncation function directly.
        let long_path = "~/projects/company/team/frontend/src/components";
        let truncated = cwd::truncate_path(long_path, 30);
        assert!(truncated.len() <= long_path.len());
        assert!(truncated.contains("..."));
        assert!(truncated.contains("src"));
        assert!(truncated.contains("components"));
        assert!(truncated.starts_with("~/"));
    }

    #[test]
    fn test_cwd_empty_when_no_info() {
        let ctx = test_ctx();
        let m = cwd::CwdModule;
        let segs = m.render(&ctx);
        assert!(segs.is_empty());
    }

    // -----------------------------------------------------------------------
    // 9. Pane info module
    // -----------------------------------------------------------------------

    #[test]
    fn test_pane_info_title_same_as_session() {
        let mut ctx = test_ctx();
        ctx.metrics.pane_count = 3;
        ctx.metrics.pane_title = Some("dev".into()); // Same as session name.
        let m = pane_info::PaneInfoModule;
        let segs = m.render(&ctx);
        assert!(segs[0].text.contains("3P"));
        // Title should NOT be shown when same as session name.
        assert!(!segs[0].text.contains("dev"));
    }

    #[test]
    fn test_pane_info_title_different_from_session() {
        let mut ctx = test_ctx();
        ctx.metrics.pane_count = 2;
        ctx.metrics.pane_title = Some("vim".into());
        let m = pane_info::PaneInfoModule;
        let segs = m.render(&ctx);
        assert!(segs[0].text.contains("2P"));
        assert!(segs[0].text.contains("vim"));
    }

    // -----------------------------------------------------------------------
    // 10. Module registry
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_has_all_14_modules() {
        let reg = build_module_registry();
        assert_eq!(reg.len(), 14);
        let expected_ids = [
            "session",
            "windows",
            "datetime",
            "hostname",
            "user",
            "cwd",
            "git",
            "cpu",
            "ram",
            "battery",
            "network",
            "prefix",
            "pane_info",
            "notes",
        ];
        for id in expected_ids {
            assert!(reg.contains_key(id), "Missing module: {}", id);
        }
    }

    #[test]
    fn test_render_module_unknown_returns_empty() {
        let reg = build_module_registry();
        let ctx = test_ctx();
        let segs = render_module("nonexistent", &ctx, &reg);
        assert!(segs.is_empty());
    }

    #[test]
    fn test_render_module_via_registry() {
        let reg = build_module_registry();
        let ctx = test_ctx();
        let segs = render_module("session", &ctx, &reg);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("dev"));
    }

    // -----------------------------------------------------------------------
    // 11. Hostname / User / Network / Notes / RAM (empty when no info)
    // -----------------------------------------------------------------------

    #[test]
    fn test_hostname_empty_when_no_info() {
        let ctx = test_ctx();
        let m = hostname::HostnameModule;
        let segs = m.render(&ctx);
        assert!(segs.is_empty());
    }

    #[test]
    fn test_hostname_renders_when_present() {
        let mut ctx = test_ctx();
        ctx.metrics.hostname = Some("myhost".into());
        let m = hostname::HostnameModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("myhost"));
    }

    #[test]
    fn test_user_empty_when_no_info() {
        let ctx = test_ctx();
        let m = user::UserModule;
        let segs = m.render(&ctx);
        assert!(segs.is_empty());
    }

    #[test]
    fn test_network_empty_when_no_info() {
        let ctx = test_ctx();
        let m = network::NetworkModule;
        let segs = m.render(&ctx);
        assert!(segs.is_empty());
    }

    #[test]
    fn test_network_renders_when_present() {
        let mut ctx = test_ctx();
        ctx.metrics.network = Some(NetworkInfo {
            interface: "eth0".into(),
            ip: "192.168.1.100".into(),
        });
        let m = network::NetworkModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("eth0"));
        assert!(segs[0].text.contains("192.168.1.100"));
    }

    #[test]
    fn test_notes_renders_count() {
        let mut ctx = test_ctx();
        ctx.metrics.notes_count = 7;
        let m = notes::NotesModule;
        let segs = m.render(&ctx);
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains('7'));
    }

    #[test]
    fn test_ram_empty_when_no_info() {
        let ctx = test_ctx();
        let m = ram::RamModule;
        let segs = m.render(&ctx);
        assert!(segs.is_empty());
    }

    #[test]
    fn test_ram_percentage_mode() {
        let mut ctx = test_ctx();
        ctx.metrics.memory = Some(MemoryInfo {
            used_mb: 4096.0,
            total_mb: 16384.0,
            percentage: 25.0,
        });
        let m = ram::RamModule;
        let segs = m.render(&ctx);
        assert!(segs[0].text.contains("25%"));
    }

    #[test]
    fn test_ram_detail_mode() {
        let mut ctx = test_ctx();
        ctx.metrics.memory = Some(MemoryInfo {
            used_mb: 4096.0,
            total_mb: 16384.0,
            percentage: 25.0,
        });
        ctx.module_config
            .insert("showDetail".into(), toml::Value::Boolean(true));
        let m = ram::RamModule;
        let segs = m.render(&ctx);
        // 4096 MB = 4.0G, 16384 MB = 16.0G
        assert!(segs[0].text.contains("4.0G"));
        assert!(segs[0].text.contains("16.0G"));
    }
}
