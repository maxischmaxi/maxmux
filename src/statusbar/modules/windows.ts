import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

const CIRCLED_NUMBERS = [
  "\u2460",
  "\u2461",
  "\u2462",
  "\u2463",
  "\u2464",
  "\u2465",
  "\u2466",
  "\u2467",
  "\u2468",
  "\u2469",
];

export const windowsModule: StatusBarModule = {
  id: "windows",
  render(ctx: StatusBarModuleContext): Segment[] {
    const numbering = (ctx.moduleConfig.numbering as string) || "index";
    const style = (ctx.moduleConfig.style as string) || "default";
    const segments: Segment[] = [];

    for (const w of ctx.windows) {
      const colors = w.isActive
        ? ctx.themeColors.modules.activeWindow
        : ctx.themeColors.modules.inactiveWindow;

      let num: string;
      switch (numbering) {
        case "number":
          num = String(w.index + 1);
          break;
        case "circled":
          num = CIRCLED_NUMBERS[w.index] || String(w.index);
          break;
        default:
          num = String(w.index);
      }

      let text: string;
      switch (style) {
        case "bracketed":
          text = ` [${num}] ${w.name} `;
          break;
        default:
          text = w.isActive ? ` ${num}:${w.name}* ` : ` ${num}:${w.name}- `;
      }

      segments.push({
        text,
        fg: colors.fg,
        bg: colors.bg,
        bold: w.isActive,
      });
    }

    return segments;
  },
};
