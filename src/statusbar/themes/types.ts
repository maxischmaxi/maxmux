import type { ResolvedStatusBarTheme } from "../types.ts";

export interface StatusBarThemeDef {
  readonly name: string;
  resolve(): ResolvedStatusBarTheme;
}
