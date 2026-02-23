import { describe, expect, test, beforeEach } from "bun:test";
import { parsePrefixKey, InputRouter } from "./router.ts";
import type { InputAction } from "./router.ts";
import { KeybindingRegistry } from "./keybindings.ts";

// --- parsePrefixKey ---

describe("parsePrefixKey", () => {
  test("C-a returns 0x01", () => {
    expect(parsePrefixKey("C-a")).toBe(0x01);
  });

  test("C-z returns 0x1a", () => {
    expect(parsePrefixKey("C-z")).toBe(0x1a);
  });

  test("C-b returns 0x02", () => {
    expect(parsePrefixKey("C-b")).toBe(0x02);
  });

  test("single character returns charCode", () => {
    expect(parsePrefixKey("a")).toBe(97);
    expect(parsePrefixKey("z")).toBe(122);
  });
});

// --- InputRouter ---

describe("InputRouter", () => {
  let actions: InputAction[];
  let keybindings: KeybindingRegistry;
  let globalKeybindings: KeybindingRegistry;

  function createRouter(prefixKey = "C-a", timeout = 0): InputRouter {
    return new InputRouter(
      prefixKey,
      timeout,
      keybindings,
      globalKeybindings,
      (action) => actions.push(action),
    );
  }

  beforeEach(() => {
    actions = [];
    keybindings = new KeybindingRegistry();
    globalKeybindings = new KeybindingRegistry();
  });

  test("prefix key activates prefix mode", () => {
    const router = createRouter();

    router.handleInput(Buffer.from([0x01])); // C-a

    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("prefix-activated");
    expect(router.isPrefixActive()).toBe(true);
  });

  test("prefix mode: known keybinding fires command", () => {
    keybindings.set("c", { commandId: "window:create" });
    const router = createRouter();

    router.handleInput(Buffer.from([0x01])); // C-a → prefix
    actions = [];

    router.handleInput(Buffer.from([0x63])); // 'c'

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({ type: "command", commandId: "window:create" });
    expect(router.isPrefixActive()).toBe(false);
  });

  test("prefix mode: Escape cancels (prefix-timeout)", () => {
    const router = createRouter();

    router.handleInput(Buffer.from([0x01])); // C-a → prefix
    actions = [];

    router.handleInput(Buffer.from([0x1b])); // Escape

    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("prefix-timeout");
    expect(router.isPrefixActive()).toBe(false);
  });

  test("prefix mode: unknown key fires prefix-timeout (swallow)", () => {
    const router = createRouter();

    router.handleInput(Buffer.from([0x01])); // C-a
    actions = [];

    router.handleInput(Buffer.from([0x78])); // 'x' (unbound)

    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("prefix-timeout");
  });

  test("global keybinding fires command without prefix", () => {
    globalKeybindings.set("C-h", { commandId: "pane:focus-left" });
    const router = createRouter();

    router.handleInput(Buffer.from([0x08])); // C-h = 0x08

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "pane:focus-left",
    });
  });

  test("normal input passes through", () => {
    const router = createRouter();
    const data = Buffer.from("hello");

    router.handleInput(data);

    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("passthrough");
    if (actions[0]!.type === "passthrough") {
      expect(actions[0]!.data).toEqual(data);
    }
  });

  test("priority: prefix key > global keybinding", () => {
    // C-a is both prefix key and could be a global keybinding
    globalKeybindings.set("C-a", { commandId: "some:command" });
    const router = createRouter("C-a");

    router.handleInput(Buffer.from([0x01])); // C-a

    // Should activate prefix, not fire global command
    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("prefix-activated");
  });

  test("priority: prefix binding > global binding for same key", () => {
    keybindings.set("c", { commandId: "prefix:command" });
    globalKeybindings.set("c", { commandId: "global:command" });
    const router = createRouter();

    router.handleInput(Buffer.from([0x01])); // Enter prefix mode
    actions = [];

    router.handleInput(Buffer.from([0x63])); // 'c'

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "prefix:command",
    });
  });

  test("arrow keys work in prefix mode", () => {
    keybindings.set("Up", { commandId: "pane:focus-up" });
    const router = createRouter();

    router.handleInput(Buffer.from([0x01])); // C-a
    actions = [];

    router.handleInput(Buffer.from([0x1b, 0x5b, 0x41])); // Up arrow

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "pane:focus-up",
    });
  });

  test("arrow keys work as global keybinding", () => {
    globalKeybindings.set("Left", { commandId: "pane:focus-left" });
    const router = createRouter();

    router.handleInput(Buffer.from([0x1b, 0x5b, 0x44])); // Left arrow

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "pane:focus-left",
    });
  });

  test("unless: global keybinding skipped when process matches", () => {
    globalKeybindings.set("C-h", {
      commandId: "pane:focus-left",
      unless: ["nvim", "vim"],
    });
    const router = new InputRouter(
      "C-a",
      0,
      keybindings,
      globalKeybindings,
      (action) => actions.push(action),
      () => "nvim",
    );

    router.handleInput(Buffer.from([0x08])); // C-h

    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("passthrough");
  });

  test("unless: global keybinding fires when process does not match", () => {
    globalKeybindings.set("C-h", {
      commandId: "pane:focus-left",
      unless: ["nvim", "vim"],
    });
    const router = new InputRouter(
      "C-a",
      0,
      keybindings,
      globalKeybindings,
      (action) => actions.push(action),
      () => "zsh",
    );

    router.handleInput(Buffer.from([0x08])); // C-h

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "pane:focus-left",
    });
  });

  test("unless: global keybinding fires when no process info", () => {
    globalKeybindings.set("C-h", {
      commandId: "pane:focus-left",
      unless: ["nvim"],
    });
    const router = new InputRouter(
      "C-a",
      0,
      keybindings,
      globalKeybindings,
      (action) => actions.push(action),
      () => undefined,
    );

    router.handleInput(Buffer.from([0x08])); // C-h

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "pane:focus-left",
    });
  });

  test("Alt+key fires global keybinding (M-j)", () => {
    globalKeybindings.set("M-j", { commandId: "pane:focus-down" });
    const router = createRouter();

    // Alt+j = ESC (0x1b) + 'j' (0x6a)
    router.handleInput(Buffer.from([0x1b, 0x6a]));

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "pane:focus-down",
    });
  });

  test("Alt+Space fires global keybinding (M-Space)", () => {
    globalKeybindings.set("M-Space", { commandId: "command-palette" });
    const router = createRouter();

    // Alt+Space = ESC (0x1b) + Space (0x20)
    router.handleInput(Buffer.from([0x1b, 0x20]));

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "command-palette",
    });
  });

  test("Alt+key works in prefix mode (M-x)", () => {
    keybindings.set("M-x", { commandId: "pane:close" });
    const router = createRouter();

    router.handleInput(Buffer.from([0x01])); // C-a → prefix
    actions = [];

    // Alt+x = ESC (0x1b) + 'x' (0x78)
    router.handleInput(Buffer.from([0x1b, 0x78]));

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({ type: "command", commandId: "pane:close" });
  });

  test("Alt+Ctrl fires global keybinding (M-C-j)", () => {
    globalKeybindings.set("M-C-j", { commandId: "pane:focus-down" });
    const router = createRouter();

    // Alt+Ctrl+j = ESC (0x1b) + 0x0a (Ctrl+j)
    router.handleInput(Buffer.from([0x1b, 0x0a]));

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "pane:focus-down",
    });
  });

  test("unbound Alt+key passes through", () => {
    const router = createRouter();

    // Alt+z = ESC + 'z', no binding
    router.handleInput(Buffer.from([0x1b, 0x7a]));

    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("passthrough");
  });

  test("Alt+key respects unless clause", () => {
    globalKeybindings.set("M-j", {
      commandId: "pane:focus-down",
      unless: ["vim"],
    });
    const router = new InputRouter(
      "C-a",
      0,
      keybindings,
      globalKeybindings,
      (action) => actions.push(action),
      () => "vim",
    );

    router.handleInput(Buffer.from([0x1b, 0x6a])); // Alt+j

    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("passthrough");
  });

  test("C-Space fires global keybinding", () => {
    globalKeybindings.set("C-Space", { commandId: "copy-mode:enter" });
    const router = createRouter();

    // Ctrl+Space = 0x00 (NUL byte)
    router.handleInput(Buffer.from([0x00]));

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "copy-mode:enter",
    });
  });

  test("C-Space respects unless clause", () => {
    globalKeybindings.set("C-Space", {
      commandId: "copy-mode:enter",
      unless: ["nvim"],
    });
    const router = new InputRouter(
      "C-a",
      0,
      keybindings,
      globalKeybindings,
      (action) => actions.push(action),
      () => "nvim",
    );

    router.handleInput(Buffer.from([0x00])); // C-Space

    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("passthrough");
  });

  test("arrow keys still work (not confused with Alt)", () => {
    globalKeybindings.set("M-[", { commandId: "some:command" });
    const router = createRouter();

    // Up arrow = ESC [ A (3 bytes) — should NOT match M-[
    router.handleInput(Buffer.from([0x1b, 0x5b, 0x41]));

    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("passthrough"); // no global binding for "Up"
  });

  test("unless: prefix keybinding also respects unless clause", () => {
    keybindings.set("c", {
      commandId: "window:create",
      unless: ["nvim"],
    });
    const router = new InputRouter(
      "C-a",
      0,
      keybindings,
      globalKeybindings,
      (action) => actions.push(action),
      () => "nvim",
    );

    router.handleInput(Buffer.from([0x01])); // C-a → prefix
    actions = [];

    router.handleInput(Buffer.from([0x63])); // 'c'

    // Should not fire the command — treated as unbound, fires prefix-timeout
    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("prefix-timeout");
  });
});
