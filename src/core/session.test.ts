import { describe, expect, test, beforeEach } from "bun:test";
import { SessionManager } from "./session.ts";
import type { Pane } from "./session.ts";

describe("SessionManager", () => {
  let sm: SessionManager;

  beforeEach(() => {
    sm = new SessionManager();
  });

  // --- createSession ---

  describe("createSession", () => {
    test("creates session with generated ID and given name", () => {
      const session = sm.createSession("test");
      expect(session.id).toHaveLength(8);
      expect(session.name).toBe("test");
      expect(session.windows).toEqual([]);
      expect(session.activeWindow).toBe("");
      expect(session.attachedClients).toEqual([]);
    });

    test("auto-generates name when none provided", () => {
      const session = sm.createSession();
      expect(session.name).toBe("session-0");

      const session2 = sm.createSession();
      expect(session2.name).toBe("session-1");
    });

    test("multiple sessions have different IDs", () => {
      const s1 = sm.createSession("a");
      const s2 = sm.createSession("b");
      expect(s1.id).not.toBe(s2.id);
    });
  });

  // --- getSession / getSessionByName / getDefaultSession ---

  describe("getSession", () => {
    test("finds created session by ID", () => {
      const session = sm.createSession("test");
      expect(sm.getSession(session.id)).toBe(session);
    });

    test("returns undefined for non-existent ID", () => {
      expect(sm.getSession("nonexistent")).toBeUndefined();
    });
  });

  describe("getSessionByName", () => {
    test("finds session by name", () => {
      const session = sm.createSession("myname");
      expect(sm.getSessionByName("myname")).toBe(session);
    });

    test("returns undefined for non-existent name", () => {
      expect(sm.getSessionByName("nope")).toBeUndefined();
    });
  });

  describe("getDefaultSession", () => {
    test("returns first created session", () => {
      const first = sm.createSession("first");
      sm.createSession("second");
      expect(sm.getDefaultSession()).toBe(first);
    });

    test("returns undefined when no sessions exist", () => {
      expect(sm.getDefaultSession()).toBeUndefined();
    });
  });

  // --- addWindow ---

  describe("addWindow", () => {
    test("adds window to session and sets activeWindow", () => {
      const session = sm.createSession("test");
      const win = sm.addWindow(session.id);

      expect(win).not.toBeNull();
      expect(win!.id).toHaveLength(8);
      expect(session.windows).toHaveLength(1);
      expect(session.activeWindow).toBe(win!.id);
    });

    test("returns null for invalid sessionId", () => {
      expect(sm.addWindow("nonexistent")).toBeNull();
    });

    test("window has leaf layout", () => {
      const session = sm.createSession("test");
      const win = sm.addWindow(session.id)!;

      expect(win.layout.type).toBe("leaf");
      if (win.layout.type === "leaf") {
        expect(win.layout.paneId).toBe(win.activePane);
      }
    });

    test("first window becomes activeWindow, second does not override", () => {
      const session = sm.createSession("test");
      const win1 = sm.addWindow(session.id)!;
      const win2 = sm.addWindow(session.id)!;

      expect(session.activeWindow).toBe(win1.id);
      expect(session.windows).toHaveLength(2);
      expect(win2.id).not.toBe(win1.id);
    });
  });

  // --- switchWindow ---

  describe("switchWindow", () => {
    test("next wraps cyclically", () => {
      const session = sm.createSession("test");
      const win1 = sm.addWindow(session.id)!;
      const win2 = sm.addWindow(session.id)!;
      const win3 = sm.addWindow(session.id)!;

      session.activeWindow = win1.id;

      sm.switchWindow(session.id, "next");
      expect(session.activeWindow).toBe(win2.id);

      sm.switchWindow(session.id, "next");
      expect(session.activeWindow).toBe(win3.id);

      sm.switchWindow(session.id, "next");
      expect(session.activeWindow).toBe(win1.id); // wrap
    });

    test("previous wraps cyclically", () => {
      const session = sm.createSession("test");
      const win1 = sm.addWindow(session.id)!;
      const win2 = sm.addWindow(session.id)!;

      session.activeWindow = win1.id;

      sm.switchWindow(session.id, "previous");
      expect(session.activeWindow).toBe(win2.id); // wrap backwards

      sm.switchWindow(session.id, "previous");
      expect(session.activeWindow).toBe(win1.id);
    });

    test("no switch with only 1 window", () => {
      const session = sm.createSession("test");
      const win = sm.addWindow(session.id)!;

      sm.switchWindow(session.id, "next");
      expect(session.activeWindow).toBe(win.id);
    });
  });

  // --- removePaneFromWindow ---

  describe("removePaneFromWindow", () => {
    test("removes pane and updates activePane if needed", () => {
      const session = sm.createSession("test");
      const win = sm.addWindow(session.id)!;

      const pane1: Pane = {
        id: "pane1",
        pid: 1,
        cwd: "/",
        command: "bash",
        title: "",
      };
      const pane2: Pane = {
        id: "pane2",
        pid: 2,
        cwd: "/",
        command: "bash",
        title: "",
      };

      sm.addPaneToWindow(session.id, win.id, pane1);
      sm.addPaneToWindow(session.id, win.id, pane2);
      win.activePane = "pane1";

      sm.removePaneFromWindow(session.id, win.id, "pane1");

      expect(win.panes).toHaveLength(1);
      expect(win.panes[0]!.id).toBe("pane2");
      expect(win.activePane).toBe("pane2");
    });

    test("does not change activePane when removing non-active pane", () => {
      const session = sm.createSession("test");
      const win = sm.addWindow(session.id)!;

      const pane1: Pane = {
        id: "pane1",
        pid: 1,
        cwd: "/",
        command: "bash",
        title: "",
      };
      const pane2: Pane = {
        id: "pane2",
        pid: 2,
        cwd: "/",
        command: "bash",
        title: "",
      };

      sm.addPaneToWindow(session.id, win.id, pane1);
      sm.addPaneToWindow(session.id, win.id, pane2);
      win.activePane = "pane1";

      sm.removePaneFromWindow(session.id, win.id, "pane2");

      expect(win.activePane).toBe("pane1");
    });
  });

  // --- removeWindow ---

  describe("removeWindow", () => {
    test("removes window and updates activeWindow if needed", () => {
      const session = sm.createSession("test");
      const win1 = sm.addWindow(session.id)!;
      const win2 = sm.addWindow(session.id)!;

      session.activeWindow = win1.id;

      sm.removeWindow(session.id, win1.id);

      expect(session.windows).toHaveLength(1);
      expect(session.activeWindow).toBe(win2.id);
    });

    test("does not change activeWindow when removing non-active window", () => {
      const session = sm.createSession("test");
      const win1 = sm.addWindow(session.id)!;
      const win2 = sm.addWindow(session.id)!;

      session.activeWindow = win1.id;

      sm.removeWindow(session.id, win2.id);

      expect(session.activeWindow).toBe(win1.id);
      expect(session.windows).toHaveLength(1);
    });
  });
});
