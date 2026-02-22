import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

export const networkModule: StatusBarModule = {
  id: "network",
  render(ctx: StatusBarModuleContext): Segment[] {
    const net = ctx.metrics.network;
    if (!net) return [];

    const icon = ctx.icons ? "󰈀 " : "";
    return [
      {
        text: ` ${icon}${net.interface}: ${net.ip} `,
        fg: ctx.colors.fg,
        bg: ctx.colors.bg,
      },
    ];
  },
};
