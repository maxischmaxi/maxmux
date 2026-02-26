import { describe, expect, test } from "bun:test";
import {
  calculateLayout,
  splitLayout,
  removeFromLayout,
  findPaneInDirection,
  getAllPaneIds,
} from "./layout.ts";
import type { Rect } from "./layout.ts";
import type { LayoutNode } from "./session.ts";

// --- calculateLayout ---

describe("calculateLayout", () => {
  const bounds: Rect = { x: 0, y: 0, width: 80, height: 24 };

  test("single leaf returns exact bounds", () => {
    const node: LayoutNode = { type: "leaf", paneId: "p1" };
    const result = calculateLayout(node, bounds);

    expect(result.size).toBe(1);
    expect(result.get("p1")).toEqual(bounds);
  });

  test("horizontal split (50/50) produces correct left/right rects", () => {
    const node: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "left" },
        { type: "leaf", paneId: "right" },
      ],
    };
    const result = calculateLayout(node, bounds);

    const left = result.get("left")!;
    const right = result.get("right")!;

    expect(left.x).toBe(0);
    expect(left.width).toBe(40);
    expect(left.height).toBe(24);

    // Right starts after border (+1)
    expect(right.x).toBe(41);
    expect(right.width).toBe(39);
    expect(right.height).toBe(24);
  });

  test("vertical split (50/50) produces correct top/bottom rects", () => {
    const node: LayoutNode = {
      type: "split",
      direction: "vertical",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "top" },
        { type: "leaf", paneId: "bottom" },
      ],
    };
    const result = calculateLayout(node, bounds);

    const top = result.get("top")!;
    const bottom = result.get("bottom")!;

    expect(top.y).toBe(0);
    expect(top.height).toBe(12);
    expect(top.width).toBe(80);

    expect(bottom.y).toBe(13);
    expect(bottom.height).toBe(11);
    expect(bottom.width).toBe(80);
  });

  test("nested split (3 panes) gives all panes valid rects", () => {
    const node: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.5,
          children: [
            { type: "leaf", paneId: "b" },
            { type: "leaf", paneId: "c" },
          ],
        },
      ],
    };
    const result = calculateLayout(node, bounds);

    expect(result.size).toBe(3);
    for (const [, rect] of result) {
      expect(rect.width).toBeGreaterThan(0);
      expect(rect.height).toBeGreaterThan(0);
    }
  });

  test("asymmetric ratio (0.3) splits correctly", () => {
    const node: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.3,
      children: [
        { type: "leaf", paneId: "narrow" },
        { type: "leaf", paneId: "wide" },
      ],
    };
    const result = calculateLayout(node, bounds);

    const narrow = result.get("narrow")!;
    const wide = result.get("wide")!;

    expect(narrow.width).toBeLessThan(wide.width);
    expect(narrow.width).toBe(24); // floor(80*0.3) = 24
    expect(wide.x).toBe(25); // 24 + 1 border
  });
});

// --- splitLayout ---

describe("splitLayout", () => {
  test("split single leaf creates split node with 2 children", () => {
    const node: LayoutNode = { type: "leaf", paneId: "p1" };
    const result = splitLayout(node, "p1", "p2", "horizontal");

    expect(result.type).toBe("split");
    if (result.type === "split") {
      expect(result.direction).toBe("horizontal");
      expect(result.ratio).toBe(0.5);
      expect(result.children[0]).toEqual({ type: "leaf", paneId: "p1" });
      expect(result.children[1]).toEqual({ type: "leaf", paneId: "p2" });
    }
  });

  test("split in deeper tree finds and splits correct pane", () => {
    const node: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        { type: "leaf", paneId: "b" },
      ],
    };
    const result = splitLayout(node, "b", "c", "vertical");

    expect(result.type).toBe("split");
    if (result.type === "split") {
      // Left child unchanged
      expect(result.children[0]).toEqual({ type: "leaf", paneId: "a" });
      // Right child is now a split
      const rightChild = result.children[1];
      expect(rightChild.type).toBe("split");
      if (rightChild.type === "split") {
        expect(rightChild.direction).toBe("vertical");
        expect(rightChild.children[0]).toEqual({ type: "leaf", paneId: "b" });
        expect(rightChild.children[1]).toEqual({ type: "leaf", paneId: "c" });
      }
    }
  });

  test("non-existent paneId leaves tree unchanged", () => {
    const node: LayoutNode = { type: "leaf", paneId: "p1" };
    const result = splitLayout(node, "nonexistent", "p2", "horizontal");

    expect(result).toEqual(node);
  });
});

// --- removeFromLayout ---

describe("removeFromLayout", () => {
  test("remove only pane returns null", () => {
    const node: LayoutNode = { type: "leaf", paneId: "p1" };
    const result = removeFromLayout(node, "p1");
    expect(result).toBeNull();
  });

  test("remove from 2-pane split returns remaining leaf", () => {
    const node: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        { type: "leaf", paneId: "b" },
      ],
    };
    const result = removeFromLayout(node, "a");
    expect(result).toEqual({ type: "leaf", paneId: "b" });
  });

  test("remove from nested tree collapses correctly", () => {
    const node: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.5,
          children: [
            { type: "leaf", paneId: "b" },
            { type: "leaf", paneId: "c" },
          ],
        },
      ],
    };

    // Remove "b" → right side collapses to just "c"
    const result = removeFromLayout(node, "b");
    expect(result).not.toBeNull();
    if (result && result.type === "split") {
      expect(result.children[0]).toEqual({ type: "leaf", paneId: "a" });
      expect(result.children[1]).toEqual({ type: "leaf", paneId: "c" });
    }
  });

  test("remove non-existent pane returns tree unchanged", () => {
    const node: LayoutNode = { type: "leaf", paneId: "p1" };
    const result = removeFromLayout(node, "nonexistent");
    expect(result).toEqual(node);
  });
});

// --- findPaneInDirection ---

describe("findPaneInDirection", () => {
  test("horizontal split: left finds right, right finds left", () => {
    const rects = new Map<string, Rect>([
      ["left", { x: 0, y: 0, width: 40, height: 24 }],
      ["right", { x: 41, y: 0, width: 39, height: 24 }],
    ]);

    expect(findPaneInDirection(rects, "left", "right")).toBe("right");
    expect(findPaneInDirection(rects, "right", "left")).toBe("left");
  });

  test("returns null when no pane in given direction", () => {
    const rects = new Map<string, Rect>([
      ["left", { x: 0, y: 0, width: 40, height: 24 }],
      ["right", { x: 41, y: 0, width: 39, height: 24 }],
    ]);

    // No pane above or below in a horizontal-only split
    expect(findPaneInDirection(rects, "left", "up")).toBeNull();
    expect(findPaneInDirection(rects, "left", "down")).toBeNull();
  });

  test("multiple panes: selects nearest by Manhattan distance", () => {
    const rects = new Map<string, Rect>([
      ["a", { x: 0, y: 0, width: 20, height: 24 }],
      ["b", { x: 21, y: 0, width: 20, height: 24 }],
      ["c", { x: 42, y: 0, width: 38, height: 24 }],
    ]);

    // From "a", looking right → "b" is closer than "c"
    expect(findPaneInDirection(rects, "a", "right")).toBe("b");
  });

  test("returns null for unknown paneId", () => {
    const rects = new Map<string, Rect>([
      ["a", { x: 0, y: 0, width: 40, height: 24 }],
    ]);
    expect(findPaneInDirection(rects, "nonexistent", "right")).toBeNull();
  });

  test("3-pane L-shape: right panes can navigate left to left pane", () => {
    // Layout: 1 left, 2 right (top-right + bottom-right)
    // +--------+--------+
    // |        | top-R  |
    // |  left  +--------+
    // |        | bot-R  |
    // +--------+--------+
    const layout: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.5,
          children: [
            { type: "leaf", paneId: "b" },
            { type: "leaf", paneId: "c" },
          ],
        },
      ],
    };
    const rects = calculateLayout(layout, {
      x: 0,
      y: 0,
      width: 80,
      height: 23,
    });

    // From b (top-right), going left should reach a (left)
    expect(findPaneInDirection(rects, "b", "left")).toBe("a");
    // From c (bottom-right), going left should reach a (left)
    expect(findPaneInDirection(rects, "c", "left")).toBe("a");
    // From a (left), going right should reach one of the right panes
    expect(findPaneInDirection(rects, "a", "right")).not.toBeNull();

    // Vertical navigation on the right side
    expect(findPaneInDirection(rects, "b", "down")).toBe("c");
    expect(findPaneInDirection(rects, "c", "up")).toBe("b");
  });

  test("3-pane L-shape: full bidirectional navigation at various sizes", () => {
    const layout: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.5,
          children: [
            { type: "leaf", paneId: "b" },
            { type: "leaf", paneId: "c" },
          ],
        },
      ],
    };

    const sizes = [
      { width: 80, height: 24 },
      { width: 120, height: 40 },
      { width: 200, height: 50 },
      { width: 40, height: 12 },
    ];

    for (const { width, height } of sizes) {
      const rects = calculateLayout(layout, { x: 0, y: 0, width, height });

      // All 3 panes must exist with positive dimensions
      expect(rects.size).toBe(3);
      for (const [, rect] of rects) {
        expect(rect.width).toBeGreaterThan(0);
        expect(rect.height).toBeGreaterThan(0);
      }

      // Left ↔ Right navigation
      expect(findPaneInDirection(rects, "b", "left")).toBe("a");
      expect(findPaneInDirection(rects, "c", "left")).toBe("a");
      const rightTarget = findPaneInDirection(rects, "a", "right");
      expect(rightTarget === "b" || rightTarget === "c").toBe(true);

      // Vertical navigation on right side
      expect(findPaneInDirection(rects, "b", "down")).toBe("c");
      expect(findPaneInDirection(rects, "c", "up")).toBe("b");

      // Left pane spans full height; right panes are above/below its center
      // so "up" from a → b (top-right) and "down" from a → c (bottom-right)
      expect(findPaneInDirection(rects, "a", "up")).toBe("b");
      expect(findPaneInDirection(rects, "a", "down")).toBe("c");
    }
  });

  test("returns null for activePaneId not in paneRects", () => {
    const rects = new Map<string, Rect>([
      ["a", { x: 0, y: 0, width: 40, height: 24 }],
      ["b", { x: 41, y: 0, width: 39, height: 24 }],
    ]);

    expect(findPaneInDirection(rects, "stale-id", "left")).toBeNull();
    expect(findPaneInDirection(rects, "stale-id", "right")).toBeNull();
    expect(findPaneInDirection(rects, "stale-id", "up")).toBeNull();
    expect(findPaneInDirection(rects, "stale-id", "down")).toBeNull();
    expect(findPaneInDirection(rects, "", "left")).toBeNull();
  });

  test("preferredPaneId in correct direction is returned", () => {
    // L-shape: a(left), b(top-right), c(bottom-right)
    const layout: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.5,
          children: [
            { type: "leaf", paneId: "b" },
            { type: "leaf", paneId: "c" },
          ],
        },
      ],
    };
    const rects = calculateLayout(layout, {
      x: 0,
      y: 0,
      width: 80,
      height: 24,
    });

    // From a, going right with preferred=c → should return c (not b)
    expect(findPaneInDirection(rects, "a", "right", "c")).toBe("c");
    // From a, going right with preferred=b → should return b
    expect(findPaneInDirection(rects, "a", "right", "b")).toBe("b");
  });

  test("preferredPaneId in wrong direction is ignored", () => {
    const rects = new Map<string, Rect>([
      ["left", { x: 0, y: 0, width: 40, height: 24 }],
      ["right", { x: 41, y: 0, width: 39, height: 24 }],
    ]);

    // Preferred is to the right, but direction is left → ignored
    expect(findPaneInDirection(rects, "left", "left", "right")).toBeNull();
  });

  test("preferredPaneId farther away than best candidate is ignored", () => {
    // L-shape: from C going up, A is technically "up" but B is much closer
    const layout: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.5,
          children: [
            { type: "leaf", paneId: "b" },
            { type: "leaf", paneId: "c" },
          ],
        },
      ],
    };
    const rects = calculateLayout(layout, {
      x: 0,
      y: 0,
      width: 80,
      height: 24,
    });

    // From C going up with preferred=a → B is closer, so B should win
    expect(findPaneInDirection(rects, "c", "up", "a")).toBe("b");
  });

  test("preferredPaneId not in paneRects is ignored", () => {
    const rects = new Map<string, Rect>([
      ["a", { x: 0, y: 0, width: 40, height: 24 }],
      ["b", { x: 41, y: 0, width: 39, height: 24 }],
    ]);

    // Preferred pane doesn't exist → falls back to normal algorithm
    expect(findPaneInDirection(rects, "a", "right", "nonexistent")).toBe("b");
  });

  test("preferredPaneId === currentPaneId is ignored", () => {
    const rects = new Map<string, Rect>([
      ["a", { x: 0, y: 0, width: 40, height: 24 }],
      ["b", { x: 41, y: 0, width: 39, height: 24 }],
    ]);

    // Preferred is same as current → falls back to normal algorithm
    expect(findPaneInDirection(rects, "a", "right", "a")).toBe("b");
  });

  test("L-shape roundtrip: C→A via left, then A→C via right with preferred", () => {
    const layout: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.5,
          children: [
            { type: "leaf", paneId: "b" },
            { type: "leaf", paneId: "c" },
          ],
        },
      ],
    };
    const rects = calculateLayout(layout, {
      x: 0,
      y: 0,
      width: 80,
      height: 24,
    });

    // User is in C, navigates left to A
    const fromC = findPaneInDirection(rects, "c", "left");
    expect(fromC).toBe("a");

    // User is now in A, navigates right with preferred=c → should return c
    const backToC = findPaneInDirection(rects, "a", "right", "c");
    expect(backToC).toBe("c");

    // Same but starting from B
    const fromB = findPaneInDirection(rects, "b", "left");
    expect(fromB).toBe("a");

    // User is now in A, navigates right with preferred=b → should return b
    const backToB = findPaneInDirection(rects, "a", "right", "b");
    expect(backToB).toBe("b");
  });

  test("4-pane layout: left/right prefers same-row neighbor over closer cross-row pane", () => {
    // +--------+--------+
    // |        |   B    |
    // |   A    +---+----+
    // |        | C | D  |
    // +--------+---+----+
    const layout: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.5,
          children: [
            { type: "leaf", paneId: "b" },
            {
              type: "split",
              direction: "horizontal",
              ratio: 0.5,
              children: [
                { type: "leaf", paneId: "c" },
                { type: "leaf", paneId: "d" },
              ],
            },
          ],
        },
      ],
    };

    // Test at various terminal sizes to ensure it works regardless of dimensions
    const sizes = [
      { width: 80, height: 24 },
      { width: 160, height: 30 },
      { width: 200, height: 50 },
      { width: 120, height: 40 },
    ];

    for (const { width, height } of sizes) {
      const rects = calculateLayout(layout, { x: 0, y: 0, width, height });

      // D → left should go to C (same row), not B
      expect(findPaneInDirection(rects, "d", "left")).toBe("c");

      // C → right should go to D (same row), not B
      expect(findPaneInDirection(rects, "c", "right")).toBe("d");

      // C → left should go to A (only pane further left)
      expect(findPaneInDirection(rects, "c", "left")).toBe("a");

      // D → up should go to B (directly above)
      expect(findPaneInDirection(rects, "d", "up")).toBe("b");

      // C → up should go to B (directly above)
      expect(findPaneInDirection(rects, "c", "up")).toBe("b");
    }
  });

  test("paneRects after JSON serialization roundtrip", () => {
    // Simulates server→client transfer where Map becomes Record and back
    const layout: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.5,
          children: [
            { type: "leaf", paneId: "b" },
            { type: "leaf", paneId: "c" },
          ],
        },
      ],
    };

    const originalRects = calculateLayout(layout, {
      x: 0,
      y: 0,
      width: 120,
      height: 40,
    });

    // Simulate JSON roundtrip (server sends Record, client rebuilds Map)
    const asRecord: Record<string, Rect> = {};
    for (const [id, rect] of originalRects) {
      asRecord[id] = rect;
    }
    const json = JSON.stringify(asRecord);
    const parsed = JSON.parse(json) as Record<string, Rect>;
    const restored = new Map(Object.entries(parsed));

    // Navigation should work identically after roundtrip
    expect(findPaneInDirection(restored, "b", "left")).toBe("a");
    expect(findPaneInDirection(restored, "c", "left")).toBe("a");
    expect(findPaneInDirection(restored, "b", "down")).toBe("c");
    expect(findPaneInDirection(restored, "c", "up")).toBe("b");
  });
});

// --- getAllPaneIds ---

describe("getAllPaneIds", () => {
  test("single leaf returns [paneId]", () => {
    const node: LayoutNode = { type: "leaf", paneId: "p1" };
    expect(getAllPaneIds(node)).toEqual(["p1"]);
  });

  test("nested tree returns all IDs in left-to-right order", () => {
    const node: LayoutNode = {
      type: "split",
      direction: "horizontal",
      ratio: 0.5,
      children: [
        { type: "leaf", paneId: "a" },
        {
          type: "split",
          direction: "vertical",
          ratio: 0.5,
          children: [
            { type: "leaf", paneId: "b" },
            { type: "leaf", paneId: "c" },
          ],
        },
      ],
    };
    expect(getAllPaneIds(node)).toEqual(["a", "b", "c"]);
  });
});
