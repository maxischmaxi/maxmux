import { Terminal } from "@xterm/headless";

export interface CellData {
  char: string;
  fg: number;
  bg: number;
  attrs: number; // bold, italic, etc. as bitmask
}

export class VirtualTerminal {
  private terminal: Terminal;
  readonly id: string;

  constructor(id: string, cols: number, rows: number) {
    this.id = id;
    this.terminal = new Terminal({ cols, rows, allowProposedApi: true });
  }

  write(data: string, onProcessed?: () => void): void {
    this.terminal.write(data, onProcessed);
  }

  onWriteParsed(listener: () => void): { dispose: () => void } {
    return this.terminal.onWriteParsed(listener);
  }

  onData(listener: (data: string) => void): { dispose: () => void } {
    return this.terminal.onData(listener);
  }

  resize(cols: number, rows: number): void {
    this.terminal.resize(cols, rows);
  }

  getLine(row: number): string {
    const buffer = this.terminal.buffer.active;
    const line = buffer.getLine(buffer.baseY + row);
    if (!line) return "";
    return line.translateToString(true);
  }

  getCursorX(): number {
    return this.terminal.buffer.active.cursorX;
  }

  getCursorY(): number {
    return this.terminal.buffer.active.cursorY;
  }

  getRows(): number {
    return this.terminal.rows;
  }

  getCols(): number {
    return this.terminal.cols;
  }

  /**
   * Read the full buffer content as an array of lines.
   * Each line is an array of cell data for precise rendering.
   */
  readBuffer(): {
    char: string;
    fg: string;
    bg: string;
    bold: boolean;
    dim: boolean;
    italic: boolean;
    underline: boolean;
    inverse: boolean;
  }[][] {
    const buffer = this.terminal.buffer.active;
    const result: {
      char: string;
      fg: string;
      bg: string;
      bold: boolean;
      dim: boolean;
      italic: boolean;
      underline: boolean;
      inverse: boolean;
    }[][] = [];

    for (let y = 0; y < this.terminal.rows; y++) {
      const line = buffer.getLine(buffer.baseY + y);
      const row: {
        char: string;
        fg: string;
        bg: string;
        bold: boolean;
        dim: boolean;
        italic: boolean;
        underline: boolean;
        inverse: boolean;
      }[] = [];

      if (!line) {
        for (let x = 0; x < this.terminal.cols; x++) {
          row.push({
            char: " ",
            fg: "",
            bg: "",
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
          });
        }
        result.push(row);
        continue;
      }

      for (let x = 0; x < this.terminal.cols; x++) {
        const cell = line.getCell(x);
        if (!cell) {
          row.push({
            char: " ",
            fg: "",
            bg: "",
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
          });
          continue;
        }

        row.push({
          char: cell.getChars() || " ",
          fg: "",
          bg: "",
          bold: cell.isBold() !== 0,
          dim: cell.isDim() !== 0,
          italic: cell.isItalic() !== 0,
          underline: cell.isUnderline() !== 0,
          inverse: cell.isInverse() !== 0,
        });
      }

      result.push(row);
    }

    return result;
  }

  /**
   * Read lines as simple strings for basic rendering.
   */
  readLines(): string[] {
    const lines: string[] = [];
    for (let y = 0; y < this.terminal.rows; y++) {
      lines.push(this.getLine(y));
    }
    return lines;
  }

  /**
   * Render a line as ANSI-escaped string with colors and attributes.
   * Only emits style changes when they differ from the previous cell.
   */
  renderLine(row: number): string {
    const buffer = this.terminal.buffer.active;
    const line = buffer.getLine(buffer.baseY + row);
    if (!line) return "\x1b[0m" + " ".repeat(this.terminal.cols);

    const CM_RGB = 0x03000000;

    let out = "\x1b[0m";
    let pFgM = -1,
      pFgC = -1,
      pBgM = -1,
      pBgC = -1;
    let pBo = -1,
      pDi = -1,
      pIt = -1,
      pUl = -1,
      pIn = -1;

    for (let x = 0; x < this.terminal.cols; x++) {
      const cell = line.getCell(x);
      if (!cell) {
        out += " ";
        continue;
      }

      if (cell.getWidth() === 0) continue;

      const fgM = cell.getFgColorMode();
      const fgC = cell.getFgColor();
      const bgM = cell.getBgColorMode();
      const bgC = cell.getBgColor();
      const bo = cell.isBold();
      const di = cell.isDim();
      const it = cell.isItalic();
      const ul = cell.isUnderline();
      const inv = cell.isInverse();

      if (
        fgM !== pFgM ||
        fgC !== pFgC ||
        bgM !== pBgM ||
        bgC !== pBgC ||
        bo !== pBo ||
        di !== pDi ||
        it !== pIt ||
        ul !== pUl ||
        inv !== pIn
      ) {
        out += "\x1b[0m";
        if (bo) out += "\x1b[1m";
        if (di) out += "\x1b[2m";
        if (it) out += "\x1b[3m";
        if (ul) out += "\x1b[4m";
        if (inv) out += "\x1b[7m";

        if (fgM === CM_RGB) {
          out += `\x1b[38;2;${(fgC >> 16) & 0xff};${(fgC >> 8) & 0xff};${fgC & 0xff}m`;
        } else if (fgM) {
          out += `\x1b[38;5;${fgC}m`;
        }

        if (bgM === CM_RGB) {
          out += `\x1b[48;2;${(bgC >> 16) & 0xff};${(bgC >> 8) & 0xff};${bgC & 0xff}m`;
        } else if (bgM) {
          out += `\x1b[48;5;${bgC}m`;
        }

        pFgM = fgM;
        pFgC = fgC;
        pBgM = bgM;
        pBgC = bgC;
        pBo = bo;
        pDi = di;
        pIt = it;
        pUl = ul;
        pIn = inv;
      }

      out += cell.getChars() || " ";
    }

    out += "\x1b[0m";
    return out;
  }

  dispose(): void {
    this.terminal.dispose();
  }
}

export class TerminalManager {
  private terminals: Map<string, VirtualTerminal> = new Map();

  create(id: string, cols: number, rows: number): VirtualTerminal {
    const term = new VirtualTerminal(id, cols, rows);
    this.terminals.set(id, term);
    return term;
  }

  get(id: string): VirtualTerminal | undefined {
    return this.terminals.get(id);
  }

  write(id: string, data: string): void {
    const term = this.terminals.get(id);
    if (term) term.write(data);
  }

  resize(id: string, cols: number, rows: number): void {
    const term = this.terminals.get(id);
    if (term) term.resize(cols, rows);
  }

  remove(id: string): void {
    const term = this.terminals.get(id);
    if (term) {
      term.dispose();
      this.terminals.delete(id);
    }
  }

  removeAll(): void {
    for (const term of this.terminals.values()) {
      term.dispose();
    }
    this.terminals.clear();
  }
}
