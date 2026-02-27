use chrono::Local;

use crate::modules::{config_str, ModuleContext, StatusBarModule};
use crate::types::Segment;

pub struct DatetimeModule;

/// Simple token-based format replacement.
/// Supported tokens: HH, mm, ss, YYYY, MM, DD.
fn format_datetime(fmt: &str) -> String {
    let now = Local::now();
    fmt.replace("HH", &format!("{:02}", now.format("%H")))
        .replace("mm", &format!("{:02}", now.format("%M")))
        .replace("ss", &format!("{:02}", now.format("%S")))
        .replace("YYYY", &format!("{}", now.format("%Y")))
        .replace("MM", &format!("{:02}", now.format("%m")))
        .replace("DD", &format!("{:02}", now.format("%d")))
}

impl StatusBarModule for DatetimeModule {
    fn id(&self) -> &str {
        "datetime"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let fmt = config_str(ctx, "format", "HH:mm");
        let formatted = format_datetime(&fmt);
        let text = if ctx.icons {
            format!(" \u{1f550} {} ", formatted)
        } else {
            format!(" {} ", formatted)
        };
        vec![Segment::new(&text, &ctx.colors.fg, &ctx.colors.bg)]
    }
}
