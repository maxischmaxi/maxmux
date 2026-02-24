import pkg from "./package.json";

const proc = Bun.spawnSync([
  "bun",
  "build",
  "src/index.ts",
  "--compile",
  "--outfile",
  "maxmux",
  "--define",
  `__MAXMUX_VERSION__=${JSON.stringify(pkg.version)}`,
]);

if (proc.stdout.length) process.stdout.write(proc.stdout);
if (proc.stderr.length) process.stderr.write(proc.stderr);
process.exit(proc.exitCode);
