export interface ScreenCell {
  char: string;
  fg: string;
  bg: string;
  bold: boolean;
  dirty: boolean;
}

export class ScreenBuffer {
  width: number;
  height: number;
  cells: ScreenCell[][];
  private prevCells: ScreenCell[][] | null = null;

  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
    this.cells = this.createGrid(width, height);
  }

  private createGrid(width: number, height: number): ScreenCell[][] {
    const grid: ScreenCell[][] = [];
    for (let y = 0; y < height; y++) {
      const row: ScreenCell[] = [];
      for (let x = 0; x < width; x++) {
        row.push({ char: " ", fg: "", bg: "", bold: false, dirty: true });
      }
      grid.push(row);
    }
    return grid;
  }

  resize(width: number, height: number): void {
    this.width = width;
    this.height = height;
    this.prevCells = null;
    this.cells = this.createGrid(width, height);
  }

  set(
    x: number,
    y: number,
    char: string,
    fg = "",
    bg = "",
    isBold = false,
  ): void {
    if (x < 0 || x >= this.width || y < 0 || y >= this.height) return;
    const cell = this.cells[y]![x]!;
    cell.char = char;
    cell.fg = fg;
    cell.bg = bg;
    cell.bold = isBold;
    cell.dirty = true;
  }

  writeString(
    x: number,
    y: number,
    str: string,
    fg = "",
    bg = "",
    isBold = false,
  ): void {
    for (let i = 0; i < str.length && x + i < this.width; i++) {
      this.set(x + i, y, str[i]!, fg, bg, isBold);
    }
  }

  fillRow(y: number, char: string, fg = "", bg = ""): void {
    if (y < 0 || y >= this.height) return;
    for (let x = 0; x < this.width; x++) {
      this.set(x, y, char, fg, bg);
    }
  }

  fillRect(
    x: number,
    y: number,
    width: number,
    height: number,
    char: string,
    fg = "",
    bg = "",
  ): void {
    for (let row = y; row < y + height && row < this.height; row++) {
      for (let col = x; col < x + width && col < this.width; col++) {
        this.set(col, row, char, fg, bg);
      }
    }
  }

  clear(): void {
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        this.set(x, y, " ");
      }
    }
  }

  /**
   * Snapshot current state for diff-based rendering.
   */
  snapshot(): void {
    this.prevCells = this.cells.map((row) => row.map((cell) => ({ ...cell })));
  }

  /**
   * Get cells that changed since last snapshot.
   */
  getDirty(): Array<{ x: number; y: number; cell: ScreenCell }> {
    const dirty: Array<{ x: number; y: number; cell: ScreenCell }> = [];

    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        const cell = this.cells[y]![x]!;
        const prev = this.prevCells?.[y]?.[x];

        if (
          !prev ||
          cell.char !== prev.char ||
          cell.fg !== prev.fg ||
          cell.bg !== prev.bg ||
          cell.bold !== prev.bold
        ) {
          dirty.push({ x, y, cell });
        }
      }
    }

    return dirty;
  }
}
