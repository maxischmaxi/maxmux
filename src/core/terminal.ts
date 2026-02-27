import { Terminal } from "@xterm/headless";

export interface CellData {
  char: string;
  fg: number;
  bg: number;
  attrs: number; // bold, italic, etc. as bitmask
}

// Regex to match DECSCUSR (Set Cursor Style) escape sequences: CSI Ps SP q
const DECSCUSR_RE = /\x1b\[(\d*) q/g;

// Regex to match DECTCEM (Cursor Visibility) escape sequences: CSI ? 25 h/l
// Handles both standalone (\x1b[?25l) and multi-param (\x1b[?12;25l) forms
const DECTCEM_RE = /\x1b\[\?(?:\d+;)*25([hl])/g;

export class VirtualTerminal {
  private terminal: Terminal;
  readonly id: string;
  private _cursorStyle: number = 0; // 0 = default (block)
  private _cursorVisible: boolean = true; // DECTCEM: true = visible

  constructor(id: string, cols: number, rows: number, scrollback?: number) {
    this.id = id;
    this.terminal = new Terminal({
      cols,
      rows,
      scrollback: scrollback ?? 1000,
      allowProposedApi: true,
    });
  }

  write(data: string, onProcessed?: () => void): void {
    // Track DECSCUSR cursor style changes in the data stream
    DECSCUSR_RE.lastIndex = 0;
    let match: RegExpExecArray | null;
    while ((match = DECSCUSR_RE.exec(data)) !== null) {
      this._cursorStyle = match[1] ? parseInt(match[1], 10) : 0;
    }

    // Track DECTCEM cursor visibility changes
    DECTCEM_RE.lastIndex = 0;
    while ((match = DECTCEM_RE.exec(data)) !== null) {
      this._cursorVisible = match[1] === "h"; // 'h' = show, 'l' = hide
    }

    this.terminal.write(data, onProcessed);
  }

  getCursorStyle(): number {
    return this._cursorStyle;
  }

  isCursorVisible(): boolean {
    return this._cursorVisible;
  }

  /** Check if the application in this terminal has enabled mouse tracking */
  isMouseTrackingActive(): boolean {
    return this.terminal.modes.mouseTrackingMode !== "none";
  }

  /** Check if the application in this terminal has enabled bracketed paste mode */
  isBracketedPasteActive(): boolean {
    return this.terminal.modes.bracketedPasteMode;
  }

  setCursorVisible(visible: boolean): void {
    this._cursorVisible = visible;
  }

  setCursorStyle(style: number): void {
    this._cursorStyle = style;
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

  isLineWrapped(row: number): boolean {
    const buffer = this.terminal.buffer.active;
    const line = buffer.getLine(buffer.baseY + row);
    return line?.isWrapped ?? false;
  }

  getCellChar(row: number, col: number): string {
    const buffer = this.terminal.buffer.active;
    const line = buffer.getLine(buffer.baseY + row);
    if (!line) return " ";
    const cell = line.getCell(col);
    if (!cell) return " ";
    return cell.getChars() || " ";
  }

  getTextRange(
    startRow: number,
    startCol: number,
    endRow: number,
    endCol: number,
  ): string {
    const buffer = this.terminal.buffer.active;
    const lines: string[] = [];
    for (let row = startRow; row <= endRow; row++) {
      const line = buffer.getLine(buffer.baseY + row);
      if (!line) {
        lines.push("");
        continue;
      }
      if (startRow === endRow) {
        lines.push(line.translateToString(true, startCol, endCol + 1));
      } else if (row === startRow) {
        lines.push(line.translateToString(true, startCol));
      } else if (row === endRow) {
        lines.push(line.translateToString(true, 0, endCol + 1));
      } else {
        lines.push(line.translateToString(true));
      }
    }
    return lines.join("\n");
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

  // --- Buffer access methods for copy-mode (absolute indices) ---

  /** Total number of lines in the active buffer (scrollback + viewport) */
  getBufferLength(): number {
    return this.terminal.buffer.active.length;
  }

  /** Number of lines scrolled off the top (scrollback lines above viewport) */
  getBaseY(): number {
    return this.terminal.buffer.active.baseY;
  }

  /** Render a line by absolute buffer index (0 = first scrollback line).
   * When `forceDim` is true, the SGR dim attribute (2m) is injected after
   * every internal SGR reset so that the entire line appears dimmed. */
  renderBufferLine(absoluteRow: number, forceDim = false): string {
    const buffer = this.terminal.buffer.active;
    const line = buffer.getLine(absoluteRow);
    const dimSeq = forceDim ? "\x1b[2m" : "";
    if (!line)
      return "\x1b[0m" + dimSeq + " ".repeat(this.terminal.cols) + "\x1b[0m";

    const CM_RGB = 0x03000000;

    let out = "\x1b[0m" + dimSeq;
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
        out += "\x1b[0m" + dimSeq;
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

  /** Get character at absolute buffer position */
  getBufferCellChar(absoluteRow: number, col: number): string {
    const buffer = this.terminal.buffer.active;
    const line = buffer.getLine(absoluteRow);
    if (!line) return " ";
    const cell = line.getCell(col);
    if (!cell) return " ";
    return cell.getChars() || " ";
  }

  /** Get full text of a line by absolute buffer index */
  getBufferLineText(absoluteRow: number): string {
    const buffer = this.terminal.buffer.active;
    const line = buffer.getLine(absoluteRow);
    if (!line) return "";
    return line.translateToString(true);
  }

  /** Get text range using absolute buffer indices */
  getBufferTextRange(
    startAbsRow: number,
    startCol: number,
    endAbsRow: number,
    endCol: number,
  ): string {
    const buffer = this.terminal.buffer.active;
    const lines: string[] = [];
    for (let row = startAbsRow; row <= endAbsRow; row++) {
      const line = buffer.getLine(row);
      if (!line) {
        lines.push("");
        continue;
      }
      if (startAbsRow === endAbsRow) {
        lines.push(line.translateToString(true, startCol, endCol + 1));
      } else if (row === startAbsRow) {
        lines.push(line.translateToString(true, startCol));
      } else if (row === endAbsRow) {
        lines.push(line.translateToString(true, 0, endCol + 1));
      } else {
        lines.push(line.translateToString(true));
      }
    }
    return lines.join("\n");
  }

  /**
   * Render a line as ANSI-escaped string with colors and attributes.
   * Only emits style changes when they differ from the previous cell.
   * When `forceDim` is true, the entire line is rendered with SGR dim.
   */
  renderLine(row: number, forceDim = false): string {
    const buffer = this.terminal.buffer.active;
    return this.renderBufferLine(buffer.baseY + row, forceDim);
  }

  dispose(): void {
    this.terminal.dispose();
  }
}

export class TerminalManager {
  private terminals: Map<string, VirtualTerminal> = new Map();

  create(
    id: string,
    cols: number,
    rows: number,
    scrollback?: number,
  ): VirtualTerminal {
    const term = new VirtualTerminal(id, cols, rows, scrollback);
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
