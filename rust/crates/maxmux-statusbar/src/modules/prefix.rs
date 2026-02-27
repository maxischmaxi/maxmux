use crate::modules::{ModuleContext, StatusBarModule};
use crate::types::Segment;

pub struct PrefixModule;

impl StatusBarModule for PrefixModule {
    fn id(&self) -> &str {
        "prefix"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        if ctx.prefix_active {
            let colors = &ctx.theme_colors.modules.prefix;
            let text = if ctx.icons {
                " \u{f030c} PREFIX ".to_string()
            } else {
                " PREFIX ".to_string()
            };
            vec![Segment::new(&text, &colors.fg, &colors.bg).with_bold(true)]
        } else {
            let colors = &ctx.theme_colors.modules.prefix_inactive;
            let text = if ctx.icons {
                " \u{f030c} WAIT ".to_string()
            } else {
                " WAIT ".to_string()
            };
            vec![Segment::new(&text, &colors.fg, &colors.bg)]
        }
    }
}
