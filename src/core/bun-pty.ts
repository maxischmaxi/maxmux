import { dlopen, FFIType, ptr, toBuffer, CString } from "bun:ffi";
import { resolve, dirname } from "node:path";
import { debugLog } from "../debug.ts";

// Locate the native library relative to this file
const LIB_PATH = resolve(dirname(import.meta.dir), "../native/libpty.so");

const lib = dlopen(LIB_PATH, {
  pty_spawn: {
    args: [
      FFIType.cstring, // shell
      FFIType.cstring, // cwd
      FFIType.i32, // cols
      FFIType.i32, // rows
      FFIType.ptr, // out_pid (int*)
    ],
    returns: FFIType.i32,
  },
  pty_resize: {
    args: [FFIType.i32, FFIType.i32, FFIType.i32],
    returns: FFIType.i32,
  },
  pty_read: {
    args: [FFIType.i32, FFIType.ptr, FFIType.i32],
    returns: FFIType.i32,
  },
  pty_write: {
    args: [FFIType.i32, FFIType.ptr, FFIType.i32],
    returns: FFIType.i32,
  },
  pty_close: {
    args: [FFIType.i32],
    returns: FFIType.i32,
  },
  pty_kill: {
    args: [FFIType.i32, FFIType.i32],
    returns: FFIType.i32,
  },
  pty_wait: {
    args: [FFIType.i32, FFIType.ptr],
    returns: FFIType.i32,
  },
});

const READ_BUF_SIZE = 16384;
const POLL_INTERVAL_MS = 5;

export interface BunPtyOptions {
  shell: string;
  cwd: string;
  cols: number;
  rows: number;
  onData: (data: string) => void;
  onExit: (exitCode: number) => void;
}

export class BunPty {
  readonly masterFd: number;
  readonly pid: number;
  private readBuf: Buffer;
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private onData: (data: string) => void;
  private onExit: (exitCode: number) => void;
  private dead = false;

  private constructor(
    masterFd: number,
    pid: number,
    onData: (data: string) => void,
    onExit: (exitCode: number) => void,
  ) {
    this.masterFd = masterFd;
    this.pid = pid;
    this.readBuf = Buffer.alloc(READ_BUF_SIZE);
    this.onData = onData;
    this.onExit = onExit;
  }

  static spawn(opts: BunPtyOptions): BunPty {
    const pidBuf = new Int32Array(1);
    const shellBuf = Buffer.from(opts.shell + "\0");
    const cwdBuf = Buffer.from(opts.cwd + "\0");

    const fd = lib.symbols.pty_spawn(
      ptr(shellBuf),
      ptr(cwdBuf),
      opts.cols,
      opts.rows,
      ptr(pidBuf),
    );

    debugLog(
      "pty",
      `pty_spawn(${opts.shell}, ${opts.cwd}, ${opts.cols}x${opts.rows}) = fd:${fd}`,
    );

    if (fd < 0) {
      throw new Error(`Failed to spawn PTY for ${opts.shell}`);
    }

    const pid = pidBuf[0]!;
    debugLog("pty", `spawned pid=${pid} fd=${fd}`);
    const pty = new BunPty(fd, pid, opts.onData, opts.onExit);
    pty.startPolling();
    return pty;
  }

  private startPolling(): void {
    this.pollTimer = setInterval(() => {
      this.poll();
    }, POLL_INTERVAL_MS);
  }

  private pollCount = 0;

  private poll(): void {
    if (this.dead) return;

    this.pollCount++;
    if (this.pollCount <= 5) {
      debugLog(
        "pty",
        `poll #${this.pollCount} fd=${this.masterFd} pid=${this.pid}`,
      );
    }

    // Read all available data
    for (;;) {
      const n = lib.symbols.pty_read(
        this.masterFd,
        ptr(this.readBuf),
        READ_BUF_SIZE,
      );

      if (this.pollCount <= 5) {
        debugLog("pty", `  read() = ${n}`);
      }

      if (n > 0) {
        const data = this.readBuf.toString("utf-8", 0, n);
        debugLog("pty", `  data(${n}): ${JSON.stringify(data.slice(0, 100))}`);
        this.onData(data);
        continue;
      }

      if (n === 0) {
        // No data available (EAGAIN)
        break;
      }

      // n < 0: EOF or error — child probably exited
      debugLog("pty", `  EOF/error, checking exit`);
      this.checkExit();
      return;
    }

    // Periodically check if child is still alive
    this.checkExit();
  }

  private checkExit(): void {
    if (this.dead) return;

    const codeBuf = new Int32Array(1);
    const result = lib.symbols.pty_wait(this.pid, ptr(codeBuf));

    if (result === 1) {
      // Child exited
      this.dead = true;
      this.stopPolling();

      // Drain remaining data
      for (;;) {
        const n = lib.symbols.pty_read(
          this.masterFd,
          ptr(this.readBuf),
          READ_BUF_SIZE,
        );
        if (n > 0) {
          this.onData(this.readBuf.toString("utf-8", 0, n));
        } else {
          break;
        }
      }

      lib.symbols.pty_close(this.masterFd);
      this.onExit(codeBuf[0]!);
    }
  }

  write(data: string): void {
    if (this.dead) return;
    const buf = Buffer.from(data, "binary");
    lib.symbols.pty_write(this.masterFd, ptr(buf), buf.length);
  }

  resize(cols: number, rows: number): void {
    if (this.dead) return;
    lib.symbols.pty_resize(this.masterFd, cols, rows);
  }

  kill(signal = 15): void {
    if (this.dead) return;
    lib.symbols.pty_kill(this.pid, signal);
  }

  destroy(): void {
    this.kill();
    this.stopPolling();
    if (!this.dead) {
      this.dead = true;
      lib.symbols.pty_close(this.masterFd);
    }
  }

  private stopPolling(): void {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }
}
