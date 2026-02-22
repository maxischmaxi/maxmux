import type {
  StatusBarModule,
  StatusBarModuleContext,
  Segment,
} from "../types.ts";

export const gitModule: StatusBarModule = {
  id: "git",
  render(ctx: StatusBarModuleContext): Segment[] {
    const git = ctx.metrics.git;
    if (!git) return [];

    const icon = ctx.icons ? " " : "";
    const dirty = git.dirty ? " *" : "";

    let extra = "";
    if (git.ahead > 0) extra += ` \u2191${git.ahead}`;
    if (git.behind > 0) extra += ` \u2193${git.behind}`;

    const colors = git.dirty
      ? ctx.themeColors.modules.gitDirty
      : ctx.themeColors.modules.gitClean;

    return [
      {
        text: ` ${icon}${git.branch}${dirty}${extra} `,
        fg: colors.fg,
        bg: colors.bg,
        bold: git.dirty,
      },
    ];
  },
};
