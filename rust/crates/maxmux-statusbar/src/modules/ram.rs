use crate::modules::{config_bool, ModuleContext, StatusBarModule};
use crate::types::Segment;

pub struct RamModule;

/// Format megabytes: if >= 1024 show as GB with 1 decimal, else show as MB.
fn format_mb(mb: f64) -> String {
    if mb >= 1024.0 {
        format!("{:.1}G", mb / 1024.0)
    } else {
        format!("{:.0}M", mb)
    }
}

impl StatusBarModule for RamModule {
    fn id(&self) -> &str {
        "ram"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let Some(mem) = &ctx.metrics.memory else {
            return vec![];
        };

        let show_detail = config_bool(ctx, "showDetail", false);

        let content = if show_detail {
            format!("{}/{}", format_mb(mem.used_mb), format_mb(mem.total_mb))
        } else {
            format!("{:.0}%", mem.percentage)
        };

        let text = if ctx.icons {
            format!(" \u{f035b} {} ", content)
        } else {
            format!(" {} ", content)
        };

        vec![Segment::new(&text, &ctx.colors.fg, &ctx.colors.bg)]
    }
}
