import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";
import { homedir } from "node:os";

export const cwdModule: StatusBarModule = {
  id: "cwd",
  render(ctx: StatusBarModuleContext): Segment[] {
    let cwd = ctx.metrics.cwd;
    if (!cwd) return [];

    const home = homedir();
    if (cwd.startsWith(home)) {
      cwd = "~" + cwd.slice(home.length);
    }

    // Shorten path if too long
    const maxLen = (ctx.moduleConfig.maxLength as number) || 30;
    if (cwd.length > maxLen) {
      const parts = cwd.split("/");
      if (parts.length > 3) {
        cwd = parts[0] + "/.../" + parts.slice(-2).join("/");
      }
    }

    const icon = ctx.icons ? " " : "";
    return [
      {
        text: ` ${icon}${cwd} `,
        fg: ctx.colors.fg,
        bg: ctx.colors.bg,
      },
    ];
  },
};
