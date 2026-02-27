use crate::modules::{ModuleContext, StatusBarModule};
use crate::types::Segment;

pub struct NotesModule;

impl StatusBarModule for NotesModule {
    fn id(&self) -> &str {
        "notes"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let count = ctx.metrics.notes_count;
        let text = if ctx.icons {
            format!(" \u{f1ce7} {} ", count)
        } else {
            format!(" {} ", count)
        };
        vec![Segment::new(&text, &ctx.colors.fg, &ctx.colors.bg)]
    }
}
