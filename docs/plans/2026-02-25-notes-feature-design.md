# Notes Feature Design

## Overview

Add a notes feature to MaxMux that allows users to quickly create, view, edit, and delete text notes directly within the terminal multiplexer. Notes are stored server-side in a SQLite database and displayed as UI overlays.

## Storage

- **Database**: SQLite at `~/.maxmux/notes.db`
- **Scope**: Global (not session-bound)

### Schema

```sql
CREATE TABLE notes (
  id TEXT PRIMARY KEY,
  content TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
```

### Title Derivation

Titles are computed dynamically from content (not stored):
- If first line starts with `#` -> rest of that line becomes the title
- Otherwise -> first word of the content becomes the title
- Empty content -> fallback title (e.g. "Untitled")

## Keybindings

| Key (after prefix) | Command | Description |
|---|---|---|
| `m` | `notes:create` | Open Quick Entry overlay |
| `M` | `notes:list` | Open Notes List overlay |

## Commands

- `notes:create` - Open the note editor overlay for a new note
- `notes:list` - Open the notes list overlay
- `notes:delete` - Delete a note (used internally from list)

## UI Components

### NoteEditor (Quick Entry + Edit)

- Full-screen overlay with a multi-line text input area
- Immediately in typing mode - user can start writing right away
- Reused for both creating new notes and editing existing ones
- `Ctrl+S` or `Esc` -> save and close
- Renders a box with title "New Note" or the note's derived title when editing

### NotesList

- Overlay listing all notes sorted by `updated_at` (newest first)
- Each row shows: derived title + date
- Navigation: Arrow keys / j/k
- `Enter` -> open selected note in NoteEditor
- `d` -> delete selected note (with confirmation)
- `Esc` -> close

## Data Flow

```sql
Client (keybind press)
  -> Server receives command (notes:create / notes:list)
  -> Server queries/modifies SQLite DB
  -> Server sends note data back to client
  -> Client renders overlay with received data
  -> User edits/selects
  -> Client sends save/delete request to server
  -> Server persists to SQLite
```

## Server Messages

New message types for notes:

- `notes:data` - Server sends list of notes to client
- `notes:saved` - Confirmation after save
- `notes:deleted` - Confirmation after delete

## Client Messages

- `notes:list` - Request all notes
- `notes:save` - Save a note (create or update)
- `notes:delete` - Delete a note by ID
