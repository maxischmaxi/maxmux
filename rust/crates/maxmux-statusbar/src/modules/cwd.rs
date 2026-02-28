use crate::modules::{ModuleContext, StatusBarModule, config_int};
use crate::types::Segment;

pub struct CwdModule;

/// Truncate a path to fit within max_length.
///
/// If the path (after ~ substitution) exceeds max_length, truncate to
/// `~/.../{last-2-components}`.
pub(crate) fn truncate_path(path: &str, max_length: usize) -> String {
    if path.len() <= max_length {
        return path.to_string();
    }

    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 2 {
        return path.to_string();
    }

    let last_two = &parts[parts.len() - 2..];
    let prefix = if path.starts_with('~') { "~" } else { "" };
    format!("{}/.../{}/{}", prefix, last_two[0], last_two[1])
}

/// Replace home directory prefix with ~.
fn replace_home(path: &str) -> String {
    // Try common home dir patterns.
    if let Ok(home) = std::env::var("HOME") {
        if path.starts_with(&home) {
            return format!("~{}", &path[home.len()..]);
        }
    }
    // Also handle /home/user pattern for testing.
    path.to_string()
}

impl StatusBarModule for CwdModule {
    fn id(&self) -> &str {
        "cwd"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let Some(cwd) = &ctx.metrics.cwd else {
            return vec![];
        };
        let max_length = config_int(ctx, "maxLength", 30) as usize;
        let display_path = replace_home(cwd);
        let display_path = truncate_path(&display_path, max_length);
        let text = if ctx.icons {
            format!(" \u{f024b} {} ", display_path)
        } else {
            format!(" {} ", display_path)
        };
        vec![Segment::new(&text, &ctx.colors.fg, &ctx.colors.bg)]
    }
}
