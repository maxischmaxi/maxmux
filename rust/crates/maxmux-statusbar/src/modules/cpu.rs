use crate::modules::{ModuleContext, StatusBarModule, config_bool};
use crate::types::{ColorPair, Segment};

pub struct CpuModule;

/// Get the appropriate color pair for a CPU usage percentage.
pub(crate) fn cpu_colors(ctx: &ModuleContext, usage: f64) -> ColorPair {
    if usage > 80.0 {
        ctx.theme_colors.modules.cpu_high.clone()
    } else if usage >= 50.0 {
        ctx.theme_colors.modules.cpu_med.clone()
    } else {
        ctx.theme_colors.modules.cpu_low.clone()
    }
}

/// Render a 5-character bar graph.
fn bar_graph(usage: f64) -> String {
    let filled = ((usage / 100.0) * 5.0).round() as usize;
    let filled = filled.min(5);
    let empty = 5 - filled;
    format!("{}{}", "\u{2593}".repeat(filled), "\u{2591}".repeat(empty))
}

impl StatusBarModule for CpuModule {
    fn id(&self) -> &str {
        "cpu"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let Some(cpu) = &ctx.metrics.cpu else {
            return vec![];
        };

        let show_bar = config_bool(ctx, "showBar", false);
        let colors = cpu_colors(ctx, cpu.usage);

        let bar_str = if show_bar {
            format!(" {}", bar_graph(cpu.usage))
        } else {
            String::new()
        };

        let text = if ctx.icons {
            format!(" \u{f0ee0} {:.0}%{} ", cpu.usage, bar_str)
        } else {
            format!(" {:.0}%{} ", cpu.usage, bar_str)
        };

        vec![Segment::new(&text, &colors.fg, &colors.bg)]
    }
}
