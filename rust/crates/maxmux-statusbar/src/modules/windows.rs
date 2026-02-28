use crate::modules::{ModuleContext, StatusBarModule, config_str};
use crate::types::Segment;

pub struct WindowsModule;

/// Circled number characters for indices 0-9.
const CIRCLED: [char; 10] = [
    '\u{2460}', // ①
    '\u{2461}', // ②
    '\u{2462}', // ③
    '\u{2463}', // ④
    '\u{2464}', // ⑤
    '\u{2465}', // ⑥
    '\u{2466}', // ⑦
    '\u{2467}', // ⑧
    '\u{2468}', // ⑨
    '\u{2469}', // ⑩
];

fn format_index(index: usize, numbering: &str) -> String {
    match numbering {
        "circled" => {
            if index < CIRCLED.len() {
                CIRCLED[index].to_string()
            } else {
                index.to_string()
            }
        }
        "number" => (index + 1).to_string(),
        // "index" and default
        _ => index.to_string(),
    }
}

impl StatusBarModule for WindowsModule {
    fn id(&self) -> &str {
        "windows"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let numbering = config_str(ctx, "numbering", "index");
        let style = config_str(ctx, "style", "default");

        let mut segments = Vec::new();

        for win in &ctx.windows {
            let idx = format_index(win.index, &numbering);
            let (text, colors, bold) = if win.is_active {
                let colors = &ctx.theme_colors.modules.active_window;
                let text = match style.as_str() {
                    "bracketed" => format!(" [{}] {} ", idx, win.name),
                    _ => format!(" {}:{}* ", idx, win.name),
                };
                (text, colors, true)
            } else {
                let colors = &ctx.theme_colors.modules.inactive_window;
                let text = match style.as_str() {
                    "bracketed" => format!(" [{}] {} ", idx, win.name),
                    _ => format!(" {}:{}- ", idx, win.name),
                };
                (text, colors, false)
            };

            segments.push(Segment::new(&text, &colors.fg, &colors.bg).with_bold(bold));
        }

        segments
    }
}
