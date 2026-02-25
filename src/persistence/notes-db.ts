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
    const row = this.db
      .query("SELECT * FROM notes WHERE id = ?")
      .get(id) as Note | null;
    return row ?? undefined;
  }

  update(id: string, content: string): void {
    this.db.run("UPDATE notes SET content = ?, updated_at = ? WHERE id = ?", [
      content,
      Date.now(),
      id,
    ]);
  }

  deleteById(id: string): void {
    this.db.run("DELETE FROM notes WHERE id = ?", [id]);
  }

  listAll(): Note[] {
    return this.db
      .query("SELECT * FROM notes ORDER BY updated_at DESC")
      .all() as Note[];
  }

  close(): void {
    this.db.close();
  }
}
