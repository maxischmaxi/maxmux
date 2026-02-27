use crate::modules::{ModuleContext, StatusBarModule};
use crate::types::Segment;

pub struct NetworkModule;

impl StatusBarModule for NetworkModule {
    fn id(&self) -> &str {
        "network"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let Some(net) = &ctx.metrics.network else {
            return vec![];
        };

        let text = if ctx.icons {
            format!(" \u{f0200} {}: {} ", net.interface, net.ip)
        } else {
            format!(" {}: {} ", net.interface, net.ip)
        };

        vec![Segment::new(&text, &ctx.colors.fg, &ctx.colors.bg)]
    }
}
