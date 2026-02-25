# Notes Feature Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a notes feature with SQLite storage, quick-entry editor overlay, and notes list overlay to MaxMux.

**Architecture:** Server-side SQLite database at `~/.maxmux/notes.db` stores notes. New message types enable client-server communication for CRUD operations. Two UI overlays (NoteEditor for create/edit, NotesList for listing/selecting/deleting) follow the existing overlay pattern used by SessionFinder and RenameDialog.

**Tech Stack:** TypeScript/Bun, bun:sqlite (built-in SQLite), existing ANSI rendering system

---

### Task 1: Add notes command IDs to config schema

**Files:**
- Modify: `src/config/schema.ts:3-28`

**Step 1: Add command IDs**

In `src/config/schema.ts`, add `"notes:create"` and `"notes:list"` to the `COMMAND_IDS` array (before `] as const`):

```typescript
export const COMMAND_IDS = [
  // ... existing commands ...
  "copy-mode:enter",
  "notes:create",
  "notes:list",
] as const;
```

**Step 2: Add default keybindings**

In `src/config/defaults.ts`, add keybindings to `DEFAULT_KEYBINDINGS`:

```typescript
export const DEFAULT_KEYBINDINGS: Record<string, KeybindingValue> = {
  // ... existing bindings ...
  "[": "copy-mode:enter",
  m: "notes:create",
  M: "notes:list",
};
```

**Step 3: Verify build**

Run: `bun build src/index.ts --no-bundle --outdir /tmp/maxmux-check 2>&1 | head -5`
Expected: No type errors

**Step 4: Commit**

```bash
git add src/config/schema.ts src/config/defaults.ts
git commit -m "feat(notes): add notes command IDs and default keybindings"
```

---

### Task 2: Create SQLite notes storage layer

**Files:**
- Create: `src/persistence/notes-db.ts`
- Test: `src/persistence/notes-db.test.ts`

**Step 1: Write the test**

Create `src/persistence/notes-db.test.ts`:

```typescript
import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { NotesDB, type Note } from "./notes-db.ts";
import { unlinkSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

describe("NotesDB", () => {
  let db: NotesDB;
  let dbPath: string;

  beforeEach(() => {
    dbPath = join(tmpdir(), `maxmux-test-notes-${Date.now()}.db`);
    db = new NotesDB(dbPath);
  });

  afterEach(() => {
    db.close();
    try { unlinkSync(dbPath); } catch {}
  });

  test("creates table on init", () => {
    const notes = db.listAll();
    expect(notes).toEqual([]);
  });

  test("creates and retrieves a note", () => {
    const id = db.create("Hello world");
    const note = db.getById(id);
    expect(note).toBeDefined();
    expect(note!.content).toBe("Hello world");
    expect(note!.id).toBe(id);
  });

  test("updates a note", () => {
    const id = db.create("original");
    db.update(id, "updated");
    const note = db.getById(id);
    expect(note!.content).toBe("updated");
    expect(note!.updated_at).toBeGreaterThanOrEqual(note!.created_at);
  });

  test("deletes a note", () => {
    const id = db.create("to delete");
    db.deleteById(id);
    const note = db.getById(id);
    expect(note).toBeUndefined();
  });

  test("listAll returns notes sorted by updated_at desc", () => {
    const id1 = db.create("first");
    const id2 = db.create("second");
    // Update first note so it has a newer updated_at
    db.update(id1, "first updated");
    const notes = db.listAll();
    expect(notes.length).toBe(2);
    expect(notes[0].id).toBe(id1); // most recently updated
    expect(notes[1].id).toBe(id2);
  });

  test("deriveTitle returns first word for plain text", () => {
    const id = db.create("Hello world this is a note");
    const notes = db.listAll();
    const note = notes.find(n => n.id === id)!;
    expect(deriveTitle(note.content)).toBe("Hello");
  });

  test("deriveTitle returns heading for markdown", () => {
    const id = db.create("# My Great Note\nSome content here");
    const notes = db.listAll();
    const note = notes.find(n => n.id === id)!;
    expect(deriveTitle(note.content)).toBe("My Great Note");
  });

  test("deriveTitle returns Untitled for empty content", () => {
    expect(deriveTitle("")).toBe("Untitled");
    expect(deriveTitle("   ")).toBe("Untitled");
  });
});

// Import the function after defining tests so it's clear what we're testing
import { deriveTitle } from "./notes-db.ts";
```

**Step 2: Run tests to verify they fail**

Run: `bun test src/persistence/notes-db.test.ts`
Expected: FAIL — module not found

**Step 3: Implement the notes database**

Create `src/persistence/notes-db.ts`:

```typescript
import { Database } from "bun:sqlite";
import { randomUUID } from "node:crypto";

export interface Note {
  id: string;
  content: string;
  created_at: number;
  updated_at: number;
}

export function deriveTitle(content: string): string {
  const trimmed = content.trim();
  if (!trimmed) return "Untitled";

  const firstLine = trimmed.split("\n")[0];
  if (firstLine.startsWith("# ")) {
    const heading = firstLine.slice(2).trim();
    return heading || "Untitled";
  }

  const firstWord = trimmed.split(/\s+/)[0];
  return firstWord || "Untitled";
}

export class NotesDB {
  private db: Database;

  constructor(dbPath: string) {
    this.db = new Database(dbPath, { create: true });
    this.db.exec("PRAGMA journal_mode=WAL");
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS notes (
        id TEXT PRIMARY KEY,
        content TEXT NOT NULL DEFAULT '',
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
      )
    `);
  }

  create(content: string = ""): string {
    const id = randomUUID().slice(0, 8);
    const now = Date.now();
    this.db.run(
      "INSERT INTO notes (id, content, created_at, updated_at) VALUES (?, ?, ?, ?)",
      [id, content, now, now],
    );
    return id;
  }

  getById(id: string): Note | undefined {
    return this.db.query("SELECT * FROM notes WHERE id = ?").get(id) as Note | undefined;
  }

  update(id: string, content: string): void {
    this.db.run(
      "UPDATE notes SET content = ?, updated_at = ? WHERE id = ?",
      [content, Date.now(), id],
    );
  }

  deleteById(id: string): void {
    this.db.run("DELETE FROM notes WHERE id = ?", [id]);
  }

  listAll(): Note[] {
    return this.db.query("SELECT * FROM notes ORDER BY updated_at DESC").all() as Note[];
  }

  close(): void {
    this.db.close();
  }
}
```

**Step 4: Run tests to verify they pass**

Run: `bun test src/persistence/notes-db.test.ts`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add src/persistence/notes-db.ts src/persistence/notes-db.test.ts
git commit -m "feat(notes): add SQLite notes storage layer with tests"
```

---

### Task 3: Add notes message types and server-side command handling

**Files:**
- Modify: `src/server/broadcast.ts:4-30` — add notes message types
- Modify: `src/server/handler.ts:42-55` — add notes client message types
- Modify: `src/server/handler.ts:279-307` — handle notes messages
- Modify: `src/server/handler.ts:860+` — register notes commands

**Step 1: Extend ServerMessage type**

In `src/server/broadcast.ts`, add notes message variants to the `ServerMessage` union (before the closing `;`):

```typescript
export type ServerMessage =
  // ... existing types ...
  | { type: "preview-layout"; layout: any; paneRects: Record<string, any> }
  | { type: "notes:data"; notes: Array<{ id: string; content: string; created_at: number; updated_at: number }> }
  | { type: "notes:saved"; note: { id: string; content: string; created_at: number; updated_at: number } }
  | { type: "notes:deleted"; noteId: string };
```

**Step 2: Extend ClientMessage type**

In `src/server/handler.ts`, add notes client message variants to the `ClientMessage` union (around line 42-55):

```typescript
export type ClientMessage =
  // ... existing types ...
  | { type: "notes:list" }
  | { type: "notes:save"; noteId?: string; content: string }
  | { type: "notes:delete"; noteId: string };
```

**Step 3: Initialize NotesDB in ServerHandler**

In `src/server/handler.ts`:
- Add import at top: `import { NotesDB } from "../persistence/notes-db.ts";`
- Add import: `import { join } from "node:path";` (already imported — verify)
- Add instance variable in the `ServerHandler` class:
  ```typescript
  private notesDb: NotesDB;
  ```
- Initialize in the constructor (after other initializations, use the homedir pattern from existing code):
  ```typescript
  this.notesDb = new NotesDB(join(homedir(), ".maxmux", "notes.db"));
  ```

**Step 4: Handle notes messages in handleMessage**

In `src/server/handler.ts`, add cases to the `handleMessage` switch (around line 279-307):

```typescript
case "notes:list":
  this.handleNotesList(clientId);
  break;
case "notes:save":
  this.handleNotesSave(clientId, msg.noteId, msg.content);
  break;
case "notes:delete":
  this.handleNotesDelete(clientId, msg.noteId);
  break;
```

**Step 5: Add handler methods**

Add these private methods to the `ServerHandler` class (after `handleCommand`):

```typescript
private handleNotesList(clientId: string): void {
  const notes = this.notesDb.listAll();
  this.broadcaster.send(clientId, { type: "notes:data", notes });
}

private handleNotesSave(clientId: string, noteId: string | undefined, content: string): void {
  if (noteId) {
    this.notesDb.update(noteId, content);
    const note = this.notesDb.getById(noteId);
    if (note) {
      this.broadcaster.send(clientId, { type: "notes:saved", note });
    }
  } else {
    const id = this.notesDb.create(content);
    const note = this.notesDb.getById(id);
    if (note) {
      this.broadcaster.send(clientId, { type: "notes:saved", note });
    }
  }
}

private handleNotesDelete(clientId: string, noteId: string): void {
  this.notesDb.deleteById(noteId);
  this.broadcaster.send(clientId, { type: "notes:deleted", noteId });
}
```

**Step 6: Register notes commands**

In `registerDefaultCommands()` (around line 860+), add:

```typescript
this.commands.register({
  id: "notes:create",
  description: "Create a new note",
  execute: () => {
    // Handled client-side (opens overlay)
  },
});

this.commands.register({
  id: "notes:list",
  description: "Show notes list",
  execute: () => {
    // Handled client-side (opens overlay)
  },
});
```

**Step 7: Close NotesDB on server shutdown**

Find the existing `shutdown()` or cleanup method in `ServerHandler` and add `this.notesDb.close();`.

**Step 8: Verify build**

Run: `bun build src/index.ts --no-bundle --outdir /tmp/maxmux-check 2>&1 | head -5`
Expected: No type errors

**Step 9: Commit**

```bash
git add src/server/broadcast.ts src/server/handler.ts
git commit -m "feat(notes): add server-side notes message handling and SQLite integration"
```

---

### Task 4: Create NoteEditor UI overlay

**Files:**
- Create: `src/ui/NoteEditor.ts`

The NoteEditor is a multi-line text input overlay for creating/editing notes. It follows the pattern of `RenameDialog.ts` but with multi-line support.

**Step 1: Create the NoteEditor component**

Create `src/ui/NoteEditor.ts`:

```typescript
import * as ansi from "../renderer/ansi.ts";
import { renderBox, renderText } from "./components.ts";
import { deriveTitle } from "../persistence/notes-db.ts";

export interface NoteEditorState {
  noteId: string | null; // null = new note
  lines: string[];
  cursorRow: number;
  cursorCol: number;
  scrollOffset: number;
}

export function createNoteEditorState(
  noteId: string | null,
  content: string,
): NoteEditorState {
  const lines = content ? content.split("\n") : [""];
  return {
    noteId,
    lines,
    cursorRow: lines.length - 1,
    cursorCol: lines[lines.length - 1].length,
    scrollOffset: 0,
  };
}

export function getNoteContent(state: NoteEditorState): string {
  return state.lines.join("\n");
}

export function renderNoteEditor(
  state: NoteEditorState,
  cols: number,
  rows: number,
): string {
  const width = Math.min(80, cols - 4);
  const height = Math.min(30, rows - 4);
  const x = Math.floor((cols - width) / 2);
  const y = Math.floor((rows - height) / 2);

  const content = getNoteContent(state);
  const title = state.noteId ? deriveTitle(content) : "New Note";

  let out = renderBox({
    x,
    y,
    width,
    height,
    title,
    borderFg: "#89b4fa",
    bg: "#1e1e2e",
    fg: "#cdd6f4",
  });

  // Content area (inside box, excluding border rows and hint row)
  const contentHeight = height - 3; // top border + bottom border + hint row
  const contentWidth = width - 4;   // 2 border chars + 2 padding chars

  // Adjust scroll offset to keep cursor visible
  if (state.cursorRow < state.scrollOffset) {
    state.scrollOffset = state.cursorRow;
  } else if (state.cursorRow >= state.scrollOffset + contentHeight) {
    state.scrollOffset = state.cursorRow - contentHeight + 1;
  }

  // Render visible lines
  for (let i = 0; i < contentHeight; i++) {
    const lineIdx = state.scrollOffset + i;
    const line = lineIdx < state.lines.length ? state.lines[lineIdx] : "";
    const display = line.length > contentWidth
      ? line.slice(0, contentWidth)
      : line + " ".repeat(contentWidth - line.length);
    out += renderText(x + 2, y + 1 + i, display, "#cdd6f4", "#1e1e2e");
  }

  // Hint at bottom
  const hint = "Ctrl+S: save & close  Esc: save & close";
  out += renderText(
    x + Math.floor((width - hint.length) / 2),
    y + height - 1,
    hint,
    "#585b70",
  );

  // Show cursor position
  const cursorScreenRow = y + 1 + (state.cursorRow - state.scrollOffset);
  const cursorScreenCol = x + 2 + Math.min(state.cursorCol, contentWidth - 1);
  out += ansi.moveTo(cursorScreenCol, cursorScreenRow);
  out += ansi.showCursor();

  return out;
}
```

**Step 2: Verify build**

Run: `bun build src/ui/NoteEditor.ts --no-bundle --outdir /tmp/maxmux-check 2>&1 | head -5`
Expected: No errors

**Step 3: Commit**

```bash
git add src/ui/NoteEditor.ts
git commit -m "feat(notes): add NoteEditor overlay component"
```

---

### Task 5: Create NotesList UI overlay

**Files:**
- Create: `src/ui/NotesList.ts`

Follows the `SessionFinder.ts` pattern — a list with selection, search, and actions.

**Step 1: Create the NotesList component**

Create `src/ui/NotesList.ts`:

```typescript
import * as ansi from "../renderer/ansi.ts";
import { renderBox, renderList, renderText } from "./components.ts";
import { deriveTitle } from "../persistence/notes-db.ts";

export interface NotesListEntry {
  id: string;
  content: string;
  created_at: number;
  updated_at: number;
}

export interface NotesListState {
  selectedIndex: number;
  notes: NotesListEntry[];
  confirmDelete: boolean; // true when awaiting delete confirmation
}

export function createNotesListState(
  notes: NotesListEntry[],
): NotesListState {
  return {
    selectedIndex: 0,
    notes,
    confirmDelete: false,
  };
}

function formatDate(timestamp: number): string {
  const d = new Date(timestamp);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function renderNotesList(
  state: NotesListState,
  cols: number,
  rows: number,
): string {
  const maxItems = Math.min(state.notes.length, rows - 8);
  const width = Math.min(60, cols - 4);
  const height = Math.max(6, maxItems + 4);
  const x = Math.floor((cols - width) / 2);
  const y = Math.floor((rows - height) / 2);

  let out = renderBox({
    x,
    y,
    width,
    height,
    title: "Notes",
    borderFg: "#89b4fa",
    bg: "#1e1e2e",
    fg: "#cdd6f4",
  });

  if (state.notes.length === 0) {
    out += renderText(x + 2, y + 1, "No notes yet", "#585b70", "#1e1e2e");
  } else {
    const items = state.notes.slice(0, maxItems).map((n) => {
      const title = deriveTitle(n.content);
      const date = formatDate(n.updated_at);
      const maxTitleLen = width - date.length - 8;
      const displayTitle = title.length > maxTitleLen
        ? title.slice(0, maxTitleLen - 3) + "..."
        : title;
      return `${displayTitle.padEnd(maxTitleLen + 2)}${date}`;
    });

    out += renderList(
      x + 2,
      y + 1,
      items,
      state.selectedIndex,
      "#a6adc8",
      "#cdd6f4",
      "#313244",
      "#1e1e2e",
    );
  }

  // Hint / confirmation
  if (state.confirmDelete && state.notes.length > 0) {
    const hint = "Delete this note? y: yes  n: cancel";
    out += renderText(
      x + Math.floor((width - hint.length) / 2),
      y + height - 1,
      hint,
      "#f38ba8",
    );
  } else {
    const hint = "Enter: open  d: delete  Esc: close";
    out += renderText(
      x + Math.floor((width - hint.length) / 2),
      y + height - 1,
      hint,
      "#585b70",
    );
  }

  return out;
}
```

**Step 2: Verify build**

Run: `bun build src/ui/NotesList.ts --no-bundle --outdir /tmp/maxmux-check 2>&1 | head -5`
Expected: No errors

**Step 3: Commit**

```bash
git add src/ui/NotesList.ts
git commit -m "feat(notes): add NotesList overlay component"
```

---

### Task 6: Wire notes overlays into client attach

**Files:**
- Modify: `src/client/attach.ts`

This is the integration task. The client needs to:
1. Handle `notes:create` and `notes:list` commands from keybindings
2. Send/receive notes messages to/from the server
3. Show NoteEditor and NotesList overlays

**Step 1: Add imports**

At the top of `src/client/attach.ts`, add:

```typescript
import {
  type NoteEditorState,
  createNoteEditorState,
  renderNoteEditor,
  getNoteContent,
} from "../ui/NoteEditor.ts";
import {
  type NotesListState,
  type NotesListEntry,
  createNotesListState,
  renderNotesList,
} from "../ui/NotesList.ts";
```

**Step 2: Handle notes server messages**

Find where the client processes incoming `ServerMessage` (the message handler that switches on `msg.type`). Add cases for notes messages. The exact pattern depends on how existing messages are handled — look for the switch statement that handles `"state"`, `"output"`, `"layout"`, etc.

When a `notes:data` message arrives, open the NotesList overlay:

```typescript
case "notes:data":
  showNotesList(msg.notes);
  break;
case "notes:saved":
  // If notes list is open, refresh it
  break;
case "notes:deleted":
  // If notes list is open, refresh it
  break;
```

**Step 3: Add notes:create and notes:list to handleCommand**

In the `handleCommand` function (around line 923-995), add cases:

```typescript
case "notes:create":
  showNoteEditor(null, "");
  return;

case "notes:list":
  connection.send({ type: "notes:list" as any });
  return;
```

**Step 4: Implement showNoteEditor**

Add a `showNoteEditor` function following the pattern of `showRenameDialog` (line 1373+) and `showSessionFinder` (line 1244+):

```typescript
const showNoteEditor = (noteId: string | null, content: string) => {
  showingOverlay = true;
  const editorState = createNoteEditorState(noteId, content);

  const redrawEditor = () => {
    process.stdout.write(
      ansi.hideCursor() + renderNoteEditor(editorState, cols, rows),
    );
  };

  const closeEditor = (save: boolean) => {
    process.stdin.removeListener("data", onEditorData);
    if (save) {
      const content = getNoteContent(editorState);
      connection.send({
        type: "notes:save" as any,
        noteId: editorState.noteId ?? undefined,
        content,
      });
    }
    showingOverlay = false;
    process.stdout.write(ansi.clearScreen());
    renderScreen();
  };

  const onEditorData = (data: Buffer) => {
    const bytes = Array.from(data);

    // Escape — save & close
    if (bytes.length === 1 && bytes[0] === 0x1b) {
      closeEditor(true);
      return;
    }

    // Ctrl+S — save & close
    if (bytes.length === 1 && bytes[0] === 0x13) {
      closeEditor(true);
      return;
    }

    // Enter — new line
    if (bytes.length === 1 && bytes[0] === 0x0d) {
      const line = editorState.lines[editorState.cursorRow];
      const before = line.slice(0, editorState.cursorCol);
      const after = line.slice(editorState.cursorCol);
      editorState.lines[editorState.cursorRow] = before;
      editorState.lines.splice(editorState.cursorRow + 1, 0, after);
      editorState.cursorRow++;
      editorState.cursorCol = 0;
      redrawEditor();
      return;
    }

    // Backspace
    if (bytes.length === 1 && bytes[0] === 0x7f) {
      if (editorState.cursorCol > 0) {
        const line = editorState.lines[editorState.cursorRow];
        editorState.lines[editorState.cursorRow] =
          line.slice(0, editorState.cursorCol - 1) + line.slice(editorState.cursorCol);
        editorState.cursorCol--;
      } else if (editorState.cursorRow > 0) {
        // Merge with previous line
        const prevLine = editorState.lines[editorState.cursorRow - 1];
        const curLine = editorState.lines[editorState.cursorRow];
        editorState.cursorCol = prevLine.length;
        editorState.lines[editorState.cursorRow - 1] = prevLine + curLine;
        editorState.lines.splice(editorState.cursorRow, 1);
        editorState.cursorRow--;
      }
      redrawEditor();
      return;
    }

    // Arrow Up
    if (bytes.length === 3 && bytes[0] === 0x1b && bytes[1] === 0x5b && bytes[2] === 0x41) {
      if (editorState.cursorRow > 0) {
        editorState.cursorRow--;
        editorState.cursorCol = Math.min(editorState.cursorCol, editorState.lines[editorState.cursorRow].length);
      }
      redrawEditor();
      return;
    }

    // Arrow Down
    if (bytes.length === 3 && bytes[0] === 0x1b && bytes[1] === 0x5b && bytes[2] === 0x42) {
      if (editorState.cursorRow < editorState.lines.length - 1) {
        editorState.cursorRow++;
        editorState.cursorCol = Math.min(editorState.cursorCol, editorState.lines[editorState.cursorRow].length);
      }
      redrawEditor();
      return;
    }

    // Arrow Right
    if (bytes.length === 3 && bytes[0] === 0x1b && bytes[1] === 0x5b && bytes[2] === 0x43) {
      const line = editorState.lines[editorState.cursorRow];
      if (editorState.cursorCol < line.length) {
        editorState.cursorCol++;
      } else if (editorState.cursorRow < editorState.lines.length - 1) {
        editorState.cursorRow++;
        editorState.cursorCol = 0;
      }
      redrawEditor();
      return;
    }

    // Arrow Left
    if (bytes.length === 3 && bytes[0] === 0x1b && bytes[1] === 0x5b && bytes[2] === 0x44) {
      if (editorState.cursorCol > 0) {
        editorState.cursorCol--;
      } else if (editorState.cursorRow > 0) {
        editorState.cursorRow--;
        editorState.cursorCol = editorState.lines[editorState.cursorRow].length;
      }
      redrawEditor();
      return;
    }

    // Tab — insert 2 spaces
    if (bytes.length === 1 && bytes[0] === 0x09) {
      const line = editorState.lines[editorState.cursorRow];
      editorState.lines[editorState.cursorRow] =
        line.slice(0, editorState.cursorCol) + "  " + line.slice(editorState.cursorCol);
      editorState.cursorCol += 2;
      redrawEditor();
      return;
    }

    // Printable characters
    const str = data.toString("utf-8");
    const firstByte = bytes[0];
    if (str.length > 0 && firstByte !== undefined && firstByte >= 0x20 && firstByte < 0x7f) {
      const line = editorState.lines[editorState.cursorRow];
      editorState.lines[editorState.cursorRow] =
        line.slice(0, editorState.cursorCol) + str + line.slice(editorState.cursorCol);
      editorState.cursorCol += str.length;
      redrawEditor();
    }
  };

  process.stdin.on("data", onEditorData);
  redrawEditor();
};
```

**Step 5: Implement showNotesList**

```typescript
const showNotesList = (notes: NotesListEntry[]) => {
  showingOverlay = true;
  const listState = createNotesListState(notes);

  const redrawList = () => {
    process.stdout.write(
      ansi.hideCursor() + renderNotesList(listState, cols, rows),
    );
  };

  const closeList = () => {
    process.stdin.removeListener("data", onListData);
    showingOverlay = false;
    process.stdout.write(ansi.clearScreen());
    renderScreen();
  };

  const onListData = (data: Buffer) => {
    const bytes = Array.from(data);

    // Escape
    if (bytes.length === 1 && bytes[0] === 0x1b) {
      closeList();
      return;
    }

    // Delete confirmation mode
    if (listState.confirmDelete) {
      if (bytes.length === 1 && (bytes[0] === 0x79)) { // 'y'
        const note = listState.notes[listState.selectedIndex];
        if (note) {
          connection.send({ type: "notes:delete" as any, noteId: note.id });
          listState.notes.splice(listState.selectedIndex, 1);
          if (listState.selectedIndex >= listState.notes.length && listState.selectedIndex > 0) {
            listState.selectedIndex--;
          }
        }
        listState.confirmDelete = false;
        redrawList();
      } else {
        listState.confirmDelete = false;
        redrawList();
      }
      return;
    }

    // Enter — open note in editor
    if (bytes.length === 1 && bytes[0] === 0x0d) {
      const note = listState.notes[listState.selectedIndex];
      if (note) {
        closeList();
        showNoteEditor(note.id, note.content);
      }
      return;
    }

    // 'd' — delete
    if (bytes.length === 1 && bytes[0] === 0x64) {
      if (listState.notes.length > 0) {
        listState.confirmDelete = true;
        redrawList();
      }
      return;
    }

    // Arrow Up or 'k'
    if (
      (bytes.length === 3 && bytes[0] === 0x1b && bytes[1] === 0x5b && bytes[2] === 0x41) ||
      (bytes.length === 1 && bytes[0] === 0x6b)
    ) {
      if (listState.selectedIndex > 0) {
        listState.selectedIndex--;
        redrawList();
      }
      return;
    }

    // Arrow Down or 'j'
    if (
      (bytes.length === 3 && bytes[0] === 0x1b && bytes[1] === 0x5b && bytes[2] === 0x42) ||
      (bytes.length === 1 && bytes[0] === 0x6a)
    ) {
      if (listState.selectedIndex < listState.notes.length - 1) {
        listState.selectedIndex++;
        redrawList();
      }
      return;
    }
  };

  process.stdin.on("data", onListData);
  redrawList();
};
```

**Step 6: Verify build**

Run: `bun build src/index.ts --no-bundle --outdir /tmp/maxmux-check 2>&1 | head -10`
Expected: No type errors

**Step 7: Run all existing tests**

Run: `bun test`
Expected: All existing tests still pass

**Step 8: Commit**

```bash
git add src/client/attach.ts
git commit -m "feat(notes): wire NoteEditor and NotesList overlays into client"
```

---

### Task 7: Manual integration test

**Step 1: Build the binary**

Run: `bun run build`
Expected: Build succeeds

**Step 2: Test the full flow**

1. Start maxmux: `./maxmux`
2. Press `Ctrl+a` then `m` — should open NoteEditor overlay
3. Type some text, press `Esc` — should save and close
4. Press `Ctrl+a` then `M` — should open NotesList with the note
5. Select a note, press `Enter` — should open in editor
6. Press `d` then `y` — should delete the note
7. Verify `~/.maxmux/notes.db` exists

**Step 3: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "feat(notes): complete notes feature integration"
```
