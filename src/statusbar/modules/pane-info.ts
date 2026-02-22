import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

export const paneInfoModule: StatusBarModule = {
  id: "pane-info",
  render(ctx: StatusBarModuleContext): Segment[] {
    const icon = ctx.icons ? " " : "";
    const count = ctx.metrics.paneCount;
    const title = ctx.metrics.paneTitle;

    let text: string;
    if (title && title !== ctx.session.name) {
      text = ` ${icon}${count}P ${title} `;
    } else {
      text = ` ${icon}${count}P `;
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
