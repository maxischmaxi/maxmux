use crate::modules::{ModuleContext, StatusBarModule};
use crate::types::Segment;

pub struct UserModule;

impl StatusBarModule for UserModule {
    fn id(&self) -> &str {
        "user"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let Some(username) = &ctx.metrics.username else {
            return vec![];
        };
        let text = if ctx.icons {
            format!(" \u{f0135} {} ", username)
        } else {
            format!(" {} ", username)
        };
        vec![Segment::new(&text, &ctx.colors.fg, &ctx.colors.bg)]
    }
}
