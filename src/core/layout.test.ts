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
