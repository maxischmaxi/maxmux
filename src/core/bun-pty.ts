import { debugLog } from "../debug.ts";

export interface BunPtyOptions {
  shell: string;
  cwd: string;
  cols: number;
  rows: number;
  onData: (data: string) => void;
  onExit: (exitCode: number) => void;
}

export class BunPty {
  readonly pid: number;
  private terminal: InstanceType<typeof Bun.Terminal>;
  private proc: ReturnType<typeof Bun.spawn>;
  private dead = false;

  private constructor(
    terminal: InstanceType<typeof Bun.Terminal>,
    proc: ReturnType<typeof Bun.spawn>,
  ) {
    this.terminal = terminal;
    this.proc = proc;
    this.pid = proc.pid;
  }

  static spawn(opts: BunPtyOptions): BunPty {
    const decoder = new TextDecoder("utf-8", { fatal: false });

    const terminal = new Bun.Terminal({
      cols: opts.cols,
      rows: opts.rows,
      name: "xterm-256color",
      data(_term: InstanceType<typeof Bun.Terminal>, chunk: Uint8Array) {
        opts.onData(decoder.decode(chunk, { stream: true }));
      },
    });

    // Wrap with setsid -c to establish a controlling terminal.
    // Bun.Terminal does not call setsid() + ioctl(TIOCSCTTY) in the child,
    // so /dev/tty is inaccessible when the server runs as a daemon.
    // Programs like fzf, sudo, ssh depend on /dev/tty for direct I/O.
    // setsid -c: creates new session + sets PTY slave as controlling terminal.
    // In our daemon context setsid does NOT fork (child is not a PGID leader),
    // so the PID is preserved after exec.
    const proc = Bun.spawn(["setsid", "-c", opts.shell], {
      terminal,
      cwd: opts.cwd,
      env: { ...process.env, COLORTERM: "truecolor" },
    });

    const pty = new BunPty(terminal, proc);

    debugLog(
      "pty",
      `spawned pid=${proc.pid} shell=${opts.shell} cwd=${opts.cwd} ${opts.cols}x${opts.rows}`,
    );

    proc.exited.then((code: number) => {
      if (!pty.dead) {
        pty.dead = true;
        terminal.close();
        opts.onExit(code);
      }
    });

    return pty;
  }

  write(data: string): void {
    if (!this.dead) {
      this.terminal.write(Buffer.from(data, "binary"));
    }
  }

  resize(cols: number, rows: number): void {
    if (!this.dead) {
      this.terminal.resize(cols, rows);
    }
  }

  kill(signal = 15): void {
    if (!this.dead) {
      this.proc.kill(signal);
    }
  }

  destroy(): void {
    this.kill();
    if (!this.dead) {
      this.dead = true;
      this.terminal.close();
    }
  }
}
