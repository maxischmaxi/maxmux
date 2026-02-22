import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

function makeBar(percentage: number, width: number = 5): string {
  const filled = Math.round((percentage / 100) * width);
  return "\u2588".repeat(filled) + "\u2591".repeat(width - filled);
}

export const cpuModule: StatusBarModule = {
  id: "cpu",
  render(ctx: StatusBarModuleContext): Segment[] {
    const usage = ctx.metrics.cpu.usage;
    const showBar = ctx.moduleConfig.showBar as boolean;
    const icon = ctx.icons ? "󰻠 " : "";

    let colors;
    if (usage > 80) {
      colors = ctx.themeColors.modules.cpuHigh;
    } else if (usage > 50) {
      colors = ctx.themeColors.modules.cpuMed;
    } else {
      colors = ctx.themeColors.modules.cpuLow;
    }

    const pct = `${Math.round(usage)}%`;
    const bar = showBar ? ` ${makeBar(usage)}` : "";

    return [
      {
        text: ` ${icon}${pct}${bar} `,
        fg: colors.fg,
        bg: colors.bg,
      },
    ];
  },
};
