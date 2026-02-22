import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

export const hostnameModule: StatusBarModule = {
  id: "hostname",
  render(ctx: StatusBarModuleContext): Segment[] {
    if (!ctx.metrics.hostname) return [];
    const icon = ctx.icons ? "󰒋 " : "";
    return [
      {
        text: ` ${icon}${ctx.metrics.hostname} `,
        fg: ctx.colors.fg,
        bg: ctx.colors.bg,
      },
    ];
  },
};
