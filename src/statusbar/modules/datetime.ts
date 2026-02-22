import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

function formatTime(format: string): string {
  const now = new Date();
  const h = now.getHours();
  const m = now.getMinutes();
  const s = now.getSeconds();
  const Y = now.getFullYear();
  const M = now.getMonth() + 1;
  const D = now.getDate();

  return format
    .replace("HH", String(h).padStart(2, "0"))
    .replace("mm", String(m).padStart(2, "0"))
    .replace("ss", String(s).padStart(2, "0"))
    .replace("YYYY", String(Y))
    .replace("MM", String(M).padStart(2, "0"))
    .replace("DD", String(D).padStart(2, "0"));
}

export const datetimeModule: StatusBarModule = {
  id: "datetime",
  render(ctx: StatusBarModuleContext): Segment[] {
    const format = (ctx.moduleConfig.format as string) || "HH:mm";
    const icon = ctx.icons ? " " : "";
    return [
      {
        text: ` ${icon}${formatTime(format)} `,
        fg: ctx.colors.fg,
        bg: ctx.colors.bg,
      },
    ];
  },
};
