import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

export const notesModule: StatusBarModule = {
  id: "notes",
  render(ctx: StatusBarModuleContext): Segment[] {
    const count = ctx.metrics.notesCount;
    const icon = ctx.icons ? "󱓧 " : "";
    return [
      {
        text: ` ${icon}${count} `,
        fg: ctx.colors.fg,
        bg: ctx.colors.bg,
      },
    ];
  },
};
