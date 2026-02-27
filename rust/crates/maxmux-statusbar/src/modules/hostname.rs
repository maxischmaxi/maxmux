use crate::modules::{ModuleContext, StatusBarModule};
use crate::types::Segment;

pub struct HostnameModule;

impl StatusBarModule for HostnameModule {
    fn id(&self) -> &str {
        "hostname"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let Some(hostname) = &ctx.metrics.hostname else {
            return vec![];
        };
        let text = if ctx.icons {
            format!(" \u{f049b} {} ", hostname)
        } else {
            format!(" {} ", hostname)
        };
        vec![Segment::new(&text, &ctx.colors.fg, &ctx.colors.bg)]
    }
}
