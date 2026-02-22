import type { StatusBarModule } from "../types.ts";
import { sessionModule } from "./session.ts";
import { windowsModule } from "./windows.ts";
import { datetimeModule } from "./datetime.ts";
import { hostnameModule } from "./hostname.ts";
import { userModule } from "./user.ts";
import { cwdModule } from "./cwd.ts";
import { gitModule } from "./git.ts";
import { cpuModule } from "./cpu.ts";
import { ramModule } from "./ram.ts";
import { batteryModule } from "./battery.ts";
import { networkModule } from "./network.ts";
import { prefixModule } from "./prefix.ts";
import { paneInfoModule } from "./pane-info.ts";

const ALL_MODULES: StatusBarModule[] = [
  sessionModule,
  windowsModule,
  datetimeModule,
  hostnameModule,
  userModule,
  cwdModule,
  gitModule,
  cpuModule,
  ramModule,
  batteryModule,
  networkModule,
  prefixModule,
  paneInfoModule,
];

export function buildModuleRegistry(): Map<string, StatusBarModule> {
  const registry = new Map<string, StatusBarModule>();
  for (const mod of ALL_MODULES) {
    registry.set(mod.id, mod);
  }
  return registry;
}
