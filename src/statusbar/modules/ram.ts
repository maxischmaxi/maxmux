import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

function formatMB(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)}G`;
  return `${Math.round(mb)}M`;
}

export const ramModule: StatusBarModule = {
  id: "ram",
  render(ctx: StatusBarModuleContext): Segment[] {
    const mem = ctx.metrics.memory;
    const icon = ctx.icons ? "󰍛 " : "";
    const showDetail = ctx.moduleConfig.showDetail as boolean;

    let text: string;
    if (showDetail) {
      text = ` ${icon}${formatMB(mem.usedMB)}/${formatMB(mem.totalMB)} `;
    } else {
      text = ` ${icon}${Math.round(mem.percentage)}% `;
    }

    return [
      {
        text,
        fg: ctx.colors.fg,
        bg: ctx.colors.bg,
      },
    ];
  },
};
