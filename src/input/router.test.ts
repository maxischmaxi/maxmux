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
    keybindings.set("c", "window:create");
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
    globalKeybindings.set("C-h", "pane:focus-left");
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
    globalKeybindings.set("C-a", "some:command");
    const router = createRouter("C-a");

    router.handleInput(Buffer.from([0x01])); // C-a

    // Should activate prefix, not fire global command
    expect(actions).toHaveLength(1);
    expect(actions[0]!.type).toBe("prefix-activated");
  });

  test("priority: prefix binding > global binding for same key", () => {
    keybindings.set("c", "prefix:command");
    globalKeybindings.set("c", "global:command");
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
    keybindings.set("Up", "pane:focus-up");
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
    globalKeybindings.set("Left", "pane:focus-left");
    const router = createRouter();

    router.handleInput(Buffer.from([0x1b, 0x5b, 0x44])); // Left arrow

    expect(actions).toHaveLength(1);
    expect(actions[0]).toEqual({
      type: "command",
      commandId: "pane:focus-left",
    });
  });
});
