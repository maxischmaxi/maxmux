import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

export const sessionModule: StatusBarModule = {
  id: "session",
  render(ctx: StatusBarModuleContext): Segment[] {
    const icon = ctx.icons ? " " : "";
    return [
      {
        text: ` ${icon}${ctx.session.name} `,
        fg: ctx.themeColors.modules.session.fg,
        bg: ctx.themeColors.modules.session.bg,
        bold: true,
      },
    ];
  },
};
