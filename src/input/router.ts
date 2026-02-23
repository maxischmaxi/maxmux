import type { KeybindingRegistry } from "./keybindings.ts";

export type InputAction =
  | { type: "passthrough"; data: Buffer }
  | { type: "command"; commandId: string }
  | { type: "prefix-activated" }
  | { type: "prefix-timeout" };

/**
 * Parse a prefix key string like "C-a" into the corresponding byte.
 */
export function parsePrefixKey(key: string): number {
  // C-a = Ctrl+a = 0x01, C-b = 0x02, etc.
  const match = key.match(/^C-([a-z])$/);
  if (match) {
    return match[1]!.charCodeAt(0) - 96; // 'a' - 96 = 1
  }
  return key.charCodeAt(0);
}

/**
 * Parse raw terminal input bytes into a key name for keybinding lookup.
 * Used in prefix mode — handles arrow keys and printable characters.
 */
function parseKeyName(data: Buffer): string | null {
  // Arrow keys
  if (data.length === 3 && data[0] === 0x1b && data[1] === 0x5b) {
    switch (data[2]) {
      case 0x41:
        return "Up";
      case 0x42:
        return "Down";
      case 0x43:
        return "Right";
      case 0x44:
        return "Left";
    }
  }

  // Alt/Meta combinations: ESC + printable byte
  if (data.length === 2 && data[0] === 0x1b) {
    const byte = data[1]!;
    if (byte === 0x20) return "M-Space";
    if (byte >= 33 && byte < 127) return `M-${String.fromCharCode(byte)}`;
  }

  // Single printable character
  if (data.length === 1) {
    const byte = data[0]!;
    if (byte >= 32 && byte < 127) {
      return String.fromCharCode(byte);
    }
  }

  return null;
}

/**
 * Parse raw terminal input into a key name that supports Ctrl combinations.
 * Used for global (non-prefix) keybinding matching.
 *
 * Supported formats:
 *   C-a..C-z  (Ctrl+letter, bytes 0x01-0x1a)
 *   Up, Down, Left, Right (arrow keys)
 *   Single printable characters
 */
function parseGlobalKeyName(data: Buffer): string | null {
  // Arrow keys
  if (data.length === 3 && data[0] === 0x1b && data[1] === 0x5b) {
    switch (data[2]) {
      case 0x41:
        return "Up";
      case 0x42:
        return "Down";
      case 0x43:
        return "Right";
      case 0x44:
        return "Left";
    }
  }

  // Alt/Meta combinations: ESC + byte
  if (data.length === 2 && data[0] === 0x1b) {
    const byte = data[1]!;
    // Alt+Ctrl: ESC + 0x01-0x1a → M-C-a..M-C-z
    if (byte >= 1 && byte <= 26) {
      return `M-C-${String.fromCharCode(byte + 96)}`;
    }
    if (byte === 0x20) return "M-Space";
    if (byte >= 33 && byte < 127) return `M-${String.fromCharCode(byte)}`;
  }

  if (data.length === 1) {
    const byte = data[0]!;

    // Ctrl+Space: 0x00 (NUL)
    if (byte === 0) {
      return "C-Space";
    }

    // Ctrl combinations: 0x01 (C-a) through 0x1a (C-z), skip 0x1b (Escape)
    if (byte >= 1 && byte <= 26) {
      return `C-${String.fromCharCode(byte + 96)}`;
    }

    // Printable characters
    if (byte >= 32 && byte < 127) {
      return String.fromCharCode(byte);
    }
  }

  return null;
}

export class InputRouter {
  private prefixMode = false;
  private prefixTimer: ReturnType<typeof setTimeout> | null = null;
  private prefixByte: number;
  private prefixTimeout: number;
  private keybindings: KeybindingRegistry;
  private globalKeybindings: KeybindingRegistry;
  private onAction: (action: InputAction) => void;
  private getActivePaneProcess: () => string | undefined;

  constructor(
    prefixKey: string,
    prefixTimeout: number,
    keybindings: KeybindingRegistry,
    globalKeybindings: KeybindingRegistry,
    onAction: (action: InputAction) => void,
    getActivePaneProcess: () => string | undefined = () => undefined,
  ) {
    this.prefixByte = parsePrefixKey(prefixKey);
    this.prefixTimeout = prefixTimeout;
    this.keybindings = keybindings;
    this.globalKeybindings = globalKeybindings;
    this.onAction = onAction;
    this.getActivePaneProcess = getActivePaneProcess;
  }

  handleInput(data: Buffer): void {
    if (this.prefixMode) {
      this.clearPrefixTimer();
      this.prefixMode = false;

      // Escape cancels prefix mode — nothing forwarded
      if (data.length === 1 && data[0] === 0x1b) {
        this.onAction({ type: "prefix-timeout" });
        return;
      }

      const keyName = parseKeyName(data);

      if (keyName) {
        const commandId = this.keybindings.resolve(
          keyName,
          this.getActivePaneProcess(),
        );
        if (commandId) {
          this.onAction({ type: "command", commandId });
          return;
        }
      }

      // No match — swallow input, just deactivate prefix
      this.onAction({ type: "prefix-timeout" });
      return;
    }

    // Check if this is the prefix key (takes priority over global keybindings)
    if (data.length === 1 && data[0] === this.prefixByte) {
      this.prefixMode = true;
      this.onAction({ type: "prefix-activated" });

      // 0 = no timeout, prefix stays active until key or Escape
      if (this.prefixTimeout > 0) {
        this.prefixTimer = setTimeout(() => {
          this.prefixMode = false;
          this.onAction({ type: "prefix-timeout" });
          // Send the prefix byte through since user didn't follow up
          this.onAction({
            type: "passthrough",
            data: Buffer.from([this.prefixByte]),
          });
        }, this.prefixTimeout);
      }

      return;
    }

    // Check global keybindings (no prefix required)
    const globalKeyName = parseGlobalKeyName(data);
    if (globalKeyName) {
      const commandId = this.globalKeybindings.resolve(
        globalKeyName,
        this.getActivePaneProcess(),
      );
      if (commandId) {
        this.onAction({ type: "command", commandId });
        return;
      }
    }

    // Regular input - pass through to PTY
    this.onAction({ type: "passthrough", data });
  }

  private clearPrefixTimer(): void {
    if (this.prefixTimer) {
      clearTimeout(this.prefixTimer);
      this.prefixTimer = null;
    }
  }

  isPrefixActive(): boolean {
    return this.prefixMode;
  }

  updateConfig(prefixKey: string, prefixTimeout: number): void {
    this.prefixByte = parsePrefixKey(prefixKey);
    this.prefixTimeout = prefixTimeout;
    if (this.prefixMode) {
      this.clearPrefixTimer();
      this.prefixMode = false;
    }
  }

  destroy(): void {
    this.clearPrefixTimer();
  }
}
