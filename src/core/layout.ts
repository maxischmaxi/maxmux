import type { LayoutNode } from "./session.ts";

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function calculateLayout(
  node: LayoutNode,
  bounds: Rect,
): Map<string, Rect> {
  const result = new Map<string, Rect>();

  if (node.type === "leaf") {
    result.set(node.paneId, bounds);
    return result;
  }

  const { direction, ratio, children } = node;

  let firstBounds: Rect;
  let secondBounds: Rect;

  if (direction === "horizontal") {
    // Split left-right
    const splitX = Math.floor(bounds.x + bounds.width * ratio);
    firstBounds = {
      x: bounds.x,
      y: bounds.y,
      width: splitX - bounds.x,
      height: bounds.height,
    };
    secondBounds = {
      x: splitX + 1, // +1 for border
      y: bounds.y,
      width: bounds.x + bounds.width - splitX - 1,
      height: bounds.height,
    };
  } else {
    // Split top-bottom
    const splitY = Math.floor(bounds.y + bounds.height * ratio);
    firstBounds = {
      x: bounds.x,
      y: bounds.y,
      width: bounds.width,
      height: splitY - bounds.y,
    };
    secondBounds = {
      x: bounds.x,
      y: splitY + 1, // +1 for border
      width: bounds.width,
      height: bounds.y + bounds.height - splitY - 1,
    };
  }

  const firstResult = calculateLayout(children[0], firstBounds);
  const secondResult = calculateLayout(children[1], secondBounds);

  for (const [id, rect] of firstResult) result.set(id, rect);
  for (const [id, rect] of secondResult) result.set(id, rect);

  return result;
}

export function splitLayout(
  node: LayoutNode,
  paneId: string,
  newPaneId: string,
  direction: "horizontal" | "vertical",
): LayoutNode {
  if (node.type === "leaf") {
    if (node.paneId === paneId) {
      return {
        type: "split",
        direction,
        ratio: 0.5,
        children: [
          { type: "leaf", paneId },
          { type: "leaf", paneId: newPaneId },
        ],
      };
    }
    return node;
  }

  return {
    ...node,
    children: [
      splitLayout(node.children[0], paneId, newPaneId, direction),
      splitLayout(node.children[1], paneId, newPaneId, direction),
    ],
  };
}

export function removeFromLayout(
  node: LayoutNode,
  paneId: string,
): LayoutNode | null {
  if (node.type === "leaf") {
    return node.paneId === paneId ? null : node;
  }

  const left = removeFromLayout(node.children[0], paneId);
  const right = removeFromLayout(node.children[1], paneId);

  if (!left && !right) return null;
  if (!left) return right;
  if (!right) return left;

  return { ...node, children: [left, right] };
}

export function findPaneInDirection(
  paneRects: Map<string, Rect>,
  currentPaneId: string,
  direction: "up" | "down" | "left" | "right",
): string | null {
  const currentRect = paneRects.get(currentPaneId);
  if (!currentRect) return null;

  let bestId: string | null = null;
  let bestDist = Infinity;

  const cx = currentRect.x + currentRect.width / 2;
  const cy = currentRect.y + currentRect.height / 2;

  for (const [id, rect] of paneRects) {
    if (id === currentPaneId) continue;

    const px = rect.x + rect.width / 2;
    const py = rect.y + rect.height / 2;

    let isInDirection = false;
    switch (direction) {
      case "up":
        isInDirection = py < cy;
        break;
      case "down":
        isInDirection = py > cy;
        break;
      case "left":
        isInDirection = px < cx;
        break;
      case "right":
        isInDirection = px > cx;
        break;
    }

    if (isInDirection) {
      const dist = Math.abs(px - cx) + Math.abs(py - cy);
      if (dist < bestDist) {
        bestDist = dist;
        bestId = id;
      }
    }
  }

  return bestId;
}

export function getAllPaneIds(node: LayoutNode): string[] {
  if (node.type === "leaf") return [node.paneId];
  return [
    ...getAllPaneIds(node.children[0]),
    ...getAllPaneIds(node.children[1]),
  ];
}
