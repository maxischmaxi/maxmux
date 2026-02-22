import type {
  Segment,
  StatusBarModuleContext,
  SystemMetrics,
  ResolvedStatusBarTheme,
} from "./types.ts";
import { EMPTY_METRICS } from "./types.ts";
import type { StatusBarConfig } from "../config/schema.ts";
import { buildModuleRegistry } from "./modules/index.ts";
import { resolveTheme } from "./themes/index.ts";
import { getSeparatorChars, type SeparatorStyle } from "./separators.ts";
import * as ansi from "../renderer/ansi.ts";

interface SessionInfo {
  id: string;
  name: string;
}

interface WindowInfo {
  id: string;
  name: string;
  index: number;
  paneCount: number;
  isActive: boolean;
}

export class StatusBarRenderer {
  private config: StatusBarConfig;
  private theme: ResolvedStatusBarTheme;
  private moduleRegistry = buildModuleRegistry();
  private cachedOutput = "";
  private lastContextHash = "";
  private metrics: SystemMetrics = EMPTY_METRICS;
  private fallbackColors?: { bg: string; fg: string; active: string };

  constructor(
    config: StatusBarConfig,
    fallbackColors?: { bg: string; fg: string; active: string },
  ) {
    this.config = config;
    this.fallbackColors = fallbackColors;
    this.theme = resolveTheme(config.theme, fallbackColors);
  }

  updateConfig(config: StatusBarConfig): void {
    this.config = config;
    this.theme = resolveTheme(config.theme, this.fallbackColors);
    this.lastContextHash = "";
  }

  updateMetrics(metrics: SystemMetrics): void {
    this.metrics = metrics;
  }

  render(
    session: SessionInfo,
    windows: WindowInfo[],
    prefixActive: boolean,
    cols: number,
    rows: number,
  ): string {
    if (!this.config.enabled) return "";

    const contextHash = this.computeHash(
      session,
      windows,
      prefixActive,
      cols,
      this.metrics,
    );
    if (contextHash === this.lastContextHash) {
      return this.cachedOutput;
    }
    this.lastContextHash = contextHash;

    const leftModuleIds = this.config.left;
    const rightModuleIds = this.config.right;

    const leftSegments = this.renderModules(
      leftModuleIds,
      session,
      windows,
      prefixActive,
      cols,
      rows,
    );
    const rightSegments = this.renderModules(
      rightModuleIds,
      session,
      windows,
      prefixActive,
      cols,
      rows,
    );

    const separatorChars = getSeparatorChars(
      this.config.separator.style as SeparatorStyle,
      this.config.separator.left,
      this.config.separator.right,
    );

    // When prefix is active, override bar bg with prefix highlight color
    const prefixColors = this.theme.modules.prefix;
    const barBg = prefixActive ? prefixColors.bg : this.theme.bar.bg;
    const barFg = prefixActive ? prefixColors.fg : this.theme.bar.fg;

    // When prefix is active, tint all segments with prefix highlight
    if (prefixActive) {
      for (const seg of leftSegments) {
        seg.bg = prefixColors.bg;
        seg.fg = prefixColors.fg;
        seg.bold = true;
      }
      for (const seg of rightSegments) {
        seg.bg = prefixColors.bg;
        seg.fg = prefixColors.fg;
        seg.bold = true;
      }
    }

    // Build left side with separators
    const leftAnsi = this.renderSide(
      leftSegments,
      separatorChars.left,
      barBg,
      "left",
    );
    const rightAnsi = this.renderSide(
      rightSegments,
      separatorChars.right,
      barBg,
      "right",
    );

    // Calculate widths
    const leftWidth = this.measureSegments(leftSegments, true);
    const rightWidth = this.measureSegments(rightSegments, true);

    // Position bar
    const barRow = this.config.position === "top" ? 1 : rows;

    let out = "";
    out += `\x1b[${barRow};1H`; // move to bar row
    out += ansi.resetStyle();
    out += ansi.bgHex(barBg) + ansi.fgHex(barFg);
    out += ansi.clearLine();

    // Write left side
    out += leftAnsi;

    // Fill middle with bar bg
    const middleWidth = Math.max(0, cols - leftWidth - rightWidth);
    if (middleWidth > 0) {
      out += ansi.bgHex(barBg);
      out += " ".repeat(middleWidth);
    }

    // Write right side
    out += rightAnsi;

    out += ansi.resetStyle();

    this.cachedOutput = out;
    return out;
  }

  private renderModules(
    moduleIds: string[],
    session: SessionInfo,
    windows: WindowInfo[],
    prefixActive: boolean,
    cols: number,
    rows: number,
  ): Segment[] {
    const allSegments: Segment[] = [];
    let accentIdx = 0;

    for (const moduleId of moduleIds) {
      const mod = this.moduleRegistry.get(moduleId);
      if (!mod) continue;

      const modConfig =
        (this.config.modules[moduleId] as Record<string, unknown>) || {};
      if (modConfig.enabled === false) continue;

      // Determine colors for this module
      const accentColor =
        this.theme.accents[accentIdx % this.theme.accents.length]!;
      accentIdx++;

      const colors = {
        bg: (modConfig.bg as string) || accentColor,
        fg: (modConfig.fg as string) || this.theme.bar.bg,
      };

      const ctx: StatusBarModuleContext = {
        session,
        windows,
        metrics: this.metrics,
        prefixActive,
        cols,
        rows,
        colors,
        themeColors: this.theme,
        moduleConfig: modConfig,
        icons: this.config.icons,
      };

      const segments = mod.render(ctx);
      allSegments.push(...segments);
    }

    return allSegments;
  }

  private renderSide(
    segments: Segment[],
    separatorChar: string,
    barBg: string,
    side: "left" | "right",
  ): string {
    if (segments.length === 0) return "";

    let out = "";
    const isFlat = this.config.separator.style === "flat";

    if (side === "left") {
      for (let i = 0; i < segments.length; i++) {
        const seg = segments[i]!;
        out += this.applySegmentStyle(seg);
        out += seg.text;

        // Separator after this segment
        const nextBg = i + 1 < segments.length ? segments[i + 1]!.bg : barBg;
        if (!isFlat) {
          out += ansi.resetStyle();
          out += ansi.fgHex(seg.bg) + ansi.bgHex(nextBg);
          out += separatorChar;
        } else if (i + 1 < segments.length) {
          out += ansi.resetStyle();
          out += ansi.bgHex(barBg) + separatorChar;
        }
      }
    } else {
      for (let i = 0; i < segments.length; i++) {
        const seg = segments[i]!;
        const prevBg = i === 0 ? barBg : segments[i - 1]!.bg;

        // Separator before this segment
        if (!isFlat) {
          out += ansi.resetStyle();
          out += ansi.fgHex(seg.bg) + ansi.bgHex(prevBg);
          out += separatorChar;
        } else if (i > 0) {
          out += ansi.resetStyle();
          out += ansi.bgHex(barBg) + separatorChar;
        }

        out += this.applySegmentStyle(seg);
        out += seg.text;
      }
    }

    return out;
  }

  private applySegmentStyle(seg: Segment): string {
    let out = ansi.resetStyle();
    out += ansi.fgHex(seg.fg) + ansi.bgHex(seg.bg);
    if (seg.bold) out += ansi.bold();
    if (seg.italic) out += ansi.italic();
    if (seg.dim) out += ansi.dim();
    return out;
  }

  private measureSegments(segments: Segment[], includeSeps: boolean): number {
    let width = 0;
    for (const seg of segments) {
      width += seg.text.length;
    }
    if (includeSeps && segments.length > 0) {
      width += segments.length; // one separator per segment
    }
    return width;
  }

  private computeHash(
    session: SessionInfo,
    windows: WindowInfo[],
    prefixActive: boolean,
    cols: number,
    metrics: SystemMetrics,
  ): string {
    // Simple hash based on key values
    const now = new Date();
    const timeKey = this.hasSecondsModule()
      ? `${now.getHours()}:${now.getMinutes()}:${now.getSeconds()}`
      : `${now.getHours()}:${now.getMinutes()}`;

    return [
      session.id,
      session.name,
      windows.map((w) => `${w.id}:${w.isActive}`).join(","),
      prefixActive,
      cols,
      timeKey,
      metrics.cpu.usage.toFixed(0),
      metrics.memory.percentage.toFixed(0),
      metrics.git?.branch || "",
      metrics.git?.dirty || false,
      metrics.cwd,
      metrics.battery?.level || "",
    ].join("|");
  }

  private hasSecondsModule(): boolean {
    const allModules = [...this.config.left, ...this.config.right];
    if (!allModules.includes("datetime")) return false;
    const modConfig = this.config.modules.datetime;
    if (!modConfig) return false;
    const format = (modConfig as Record<string, unknown>).format as string;
    return format?.includes("ss") || false;
  }
}
