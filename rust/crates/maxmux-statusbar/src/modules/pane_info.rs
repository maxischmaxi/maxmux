use crate::modules::{ModuleContext, StatusBarModule};
use crate::types::Segment;

pub struct PaneInfoModule;

impl StatusBarModule for PaneInfoModule {
    fn id(&self) -> &str {
        "pane_info"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let count = ctx.metrics.pane_count;

        // Show title only if different from session name.
        let show_title = ctx
            .metrics
            .pane_title
            .as_ref()
            .is_some_and(|t| t != &ctx.session.name);

        let text = if ctx.icons {
            if show_title {
                format!(
                    " \u{f0135} {}P {} ",
                    count,
                    ctx.metrics.pane_title.as_deref().unwrap_or("")
                )
            } else {
                format!(" \u{f0135} {}P ", count)
            }
        } else if show_title {
            format!(
                " {}P {} ",
                count,
                ctx.metrics.pane_title.as_deref().unwrap_or("")
            )
        } else {
            format!(" {}P ", count)
        };

        vec![Segment::new(&text, &ctx.colors.fg, &ctx.colors.bg)]
    }
}
