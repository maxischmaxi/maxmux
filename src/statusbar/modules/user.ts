import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

export const userModule: StatusBarModule = {
  id: "user",
  render(ctx: StatusBarModuleContext): Segment[] {
    if (!ctx.metrics.username) return [];
    const icon = ctx.icons ? " " : "";
    return [
      {
        text: ` ${icon}${ctx.metrics.username} `,
        fg: ctx.colors.fg,
        bg: ctx.colors.bg,
      },
    ];
  },
};
