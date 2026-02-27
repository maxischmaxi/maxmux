use crate::modules::{ModuleContext, StatusBarModule};
use crate::types::Segment;

pub struct GitModule;

impl StatusBarModule for GitModule {
    fn id(&self) -> &str {
        "git"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let Some(git) = &ctx.metrics.git else {
            return vec![];
        };

        let mut detail = String::new();
        if git.ahead > 0 {
            detail.push_str(&format!(" \u{2191}{}", git.ahead));
        }
        if git.behind > 0 {
            detail.push_str(&format!(" \u{2193}{}", git.behind));
        }

        if git.dirty {
            let colors = &ctx.theme_colors.modules.git_dirty;
            let text = if ctx.icons {
                format!(" \u{f04a2} {} *{} ", git.branch, detail)
            } else {
                format!(" {} *{} ", git.branch, detail)
            };
            vec![Segment::new(&text, &colors.fg, &colors.bg).with_bold(true)]
        } else {
            let colors = &ctx.theme_colors.modules.git_clean;
            let text = if ctx.icons {
                format!(" \u{f04a2} {}{} ", git.branch, detail)
            } else {
                format!(" {}{} ", git.branch, detail)
            };
            vec![Segment::new(&text, &colors.fg, &colors.bg)]
        }
    }
}
