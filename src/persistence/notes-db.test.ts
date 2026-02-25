import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { NotesDB, type Note, deriveTitle } from "./notes-db.ts";
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
    try {
      unlinkSync(dbPath);
    } catch {}
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
});

describe("deriveTitle", () => {
  test("returns first word for plain text", () => {
    expect(deriveTitle("Hello world this is a note")).toBe("Hello");
  });

  test("returns heading for markdown", () => {
    expect(deriveTitle("# My Great Note\nSome content here")).toBe(
      "My Great Note",
    );
  });

  test("returns Untitled for empty content", () => {
    expect(deriveTitle("")).toBe("Untitled");
    expect(deriveTitle("   ")).toBe("Untitled");
  });
});
