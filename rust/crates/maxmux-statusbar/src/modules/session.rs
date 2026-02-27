use crate::modules::{ModuleContext, StatusBarModule};
use crate::types::Segment;

pub struct SessionModule;

impl StatusBarModule for SessionModule {
    fn id(&self) -> &str {
        "session"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let colors = &ctx.theme_colors.modules.session;
        let text = if ctx.icons {
            format!(" \u{f0135} {} ", ctx.session.name)
        } else {
            format!(" {} ", ctx.session.name)
        };
        vec![Segment::new(&text, &colors.fg, &colors.bg).with_bold(true)]
    }
}
