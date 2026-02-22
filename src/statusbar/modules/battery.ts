import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

function getBatteryIcon(level: number, charging: boolean): string {
  if (charging) return "󰂄";
  if (level >= 90) return "󰁹";
  if (level >= 70) return "󰂀";
  if (level >= 50) return "󰁾";
  if (level >= 30) return "󰁼";
  if (level >= 10) return "󰁺";
  return "󰂃";
}

export const batteryModule: StatusBarModule = {
  id: "battery",
  render(ctx: StatusBarModuleContext): Segment[] {
    const bat = ctx.metrics.battery;
    if (!bat || !bat.present) return [];

    const icon = ctx.icons ? getBatteryIcon(bat.level, bat.charging) + " " : "";
    const chargingIndicator = bat.charging ? "\u26a1" : "";

    let colors;
    if (bat.level > 50) {
      colors = ctx.themeColors.modules.batteryHigh;
    } else if (bat.level > 20) {
      colors = ctx.themeColors.modules.batteryMed;
    } else {
      colors = ctx.themeColors.modules.batteryLow;
    }

    return [
      {
        text: ` ${icon}${bat.level}%${chargingIndicator} `,
        fg: colors.fg,
        bg: colors.bg,
      },
    ];
  },
};
