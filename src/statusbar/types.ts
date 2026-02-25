// StatusBar type definitions

export interface Segment {
  text: string;
  fg: string;
  bg: string;
  bold?: boolean;
  italic?: boolean;
  dim?: boolean;
}

export interface StatusBarModuleContext {
  session: { id: string; name: string };
  windows: Array<{
    id: string;
    name: string;
    index: number;
    paneCount: number;
    isActive: boolean;
  }>;
  metrics: SystemMetrics;
  prefixActive: boolean;
  cols: number;
  rows: number;
  colors: { bg: string; fg: string };
  themeColors: ResolvedStatusBarTheme;
  moduleConfig: Record<string, unknown>;
  icons: boolean;
}

export interface StatusBarModule {
  readonly id: string;
  render(ctx: StatusBarModuleContext): Segment[];
}

export interface SystemMetrics {
  hostname: string;
  username: string;
  cpu: { usage: number; count: number };
  memory: { totalMB: number; usedMB: number; percentage: number };
  battery: { present: boolean; level: number; charging: boolean } | null;
  network: { interface: string; ip: string } | null;
  git: {
    branch: string;
    dirty: boolean;
    ahead: number;
    behind: number;
  } | null;
  cwd: string;
  paneTitle: string;
  paneCount: number;
  notesCount: number;
}

export interface ResolvedStatusBarTheme {
  bar: { bg: string; fg: string };
  accents: string[];
  modules: {
    session: { fg: string; bg: string };
    activeWindow: { fg: string; bg: string };
    inactiveWindow: { fg: string; bg: string };
    gitClean: { fg: string; bg: string };
    gitDirty: { fg: string; bg: string };
    cpuLow: { fg: string; bg: string };
    cpuMed: { fg: string; bg: string };
    cpuHigh: { fg: string; bg: string };
    batteryHigh: { fg: string; bg: string };
    batteryMed: { fg: string; bg: string };
    batteryLow: { fg: string; bg: string };
    prefix: { fg: string; bg: string };
    prefixInactive: { fg: string; bg: string };
  };
}

export const EMPTY_METRICS: SystemMetrics = {
  hostname: "",
  username: "",
  cpu: { usage: 0, count: 0 },
  memory: { totalMB: 0, usedMB: 0, percentage: 0 },
  battery: null,
  network: null,
  git: null,
  cwd: "",
  paneTitle: "",
  paneCount: 0,
  notesCount: 0,
};
