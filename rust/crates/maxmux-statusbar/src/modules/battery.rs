use crate::modules::{ModuleContext, StatusBarModule};
use crate::types::{ColorPair, Segment};

pub struct BatteryModule;

/// Choose battery icon based on level and charging state.
fn battery_icon(level: u8, charging: bool) -> &'static str {
    if charging {
        "\u{f0084}" // 󰂄
    } else if level >= 90 {
        "\u{f0079}" // 󰁹
    } else if level >= 70 {
        "\u{f0080}" // 󰂀
    } else if level >= 50 {
        "\u{f007e}" // 󰁾
    } else if level >= 30 {
        "\u{f007c}" // 󰁼
    } else if level >= 10 {
        "\u{f007a}" // 󰁺
    } else {
        "\u{f0083}" // 󰂃
    }
}

/// Choose battery color pair based on level.
pub(crate) fn battery_colors(ctx: &ModuleContext, level: u8) -> ColorPair {
    if level > 50 {
        ctx.theme_colors.modules.battery_high.clone()
    } else if level >= 20 {
        ctx.theme_colors.modules.battery_med.clone()
    } else {
        ctx.theme_colors.modules.battery_low.clone()
    }
}

impl StatusBarModule for BatteryModule {
    fn id(&self) -> &str {
        "battery"
    }

    fn render(&self, ctx: &ModuleContext) -> Vec<Segment> {
        let Some(bat) = &ctx.metrics.battery else {
            return vec![];
        };

        if !bat.present {
            return vec![];
        }

        let colors = battery_colors(ctx, bat.level);
        let icon = battery_icon(bat.level, bat.charging);
        let charging_indicator = if bat.charging { "\u{26a1}" } else { "" };

        let text = format!(" {} {}%{} ", icon, bat.level, charging_indicator);

        vec![Segment::new(&text, &colors.fg, &colors.bg)]
    }
}
