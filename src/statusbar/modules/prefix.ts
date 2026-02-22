import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

export const prefixModule: StatusBarModule = {
  id: "prefix",
  render(ctx: StatusBarModuleContext): Segment[] {
    const colors = ctx.prefixActive
      ? ctx.themeColors.modules.prefix
      : ctx.themeColors.modules.prefixInactive;

    const icon = ctx.icons ? "󰌌 " : "";
    const label = ctx.prefixActive ? "PREFIX" : "WAIT";

    return [
      {
        text: ` ${icon}${label} `,
        fg: colors.fg,
        bg: colors.bg,
        bold: ctx.prefixActive,
      },
    ];
  },
};
