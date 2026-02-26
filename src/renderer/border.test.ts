import { describe, expect, test } from "bun:test";
import {
  getLineChars,
  getBorderChars,
  type LineStyle,
  type BorderStyle,
} from "./border.ts";

describe("getLineChars", () => {
  test("solid returns standard box-drawing chars", () => {
    const chars = getLineChars("solid");
    expect(chars.horizontal).toBe("─");
    expect(chars.vertical).toBe("│");
  });

  test("dashed returns dashed box-drawing chars", () => {
    const chars = getLineChars("dashed");
    expect(chars.horizontal).toBe("┄");
    expect(chars.vertical).toBe("┆");
  });

  test("dotted returns dotted box-drawing chars", () => {
    const chars = getLineChars("dotted");
    expect(chars.horizontal).toBe("┈");
    expect(chars.vertical).toBe("┊");
  });

  test("all line styles return objects with horizontal and vertical", () => {
    const styles: LineStyle[] = ["solid", "dashed", "dotted"];
    for (const style of styles) {
      const chars = getLineChars(style);
      expect(typeof chars.horizontal).toBe("string");
      expect(typeof chars.vertical).toBe("string");
      expect(chars.horizontal.length).toBe(1);
      expect(chars.vertical.length).toBe(1);
    }
  });
});

describe("getBorderChars", () => {
  test("all border styles return complete char sets", () => {
    const styles: BorderStyle[] = ["rounded", "sharp", "double", "none"];
    const requiredKeys = [
      "topLeft",
      "topRight",
      "bottomLeft",
      "bottomRight",
      "horizontal",
      "vertical",
      "teeLeft",
      "teeRight",
      "teeTop",
      "teeBottom",
      "cross",
    ];
    for (const style of styles) {
      const chars = getBorderChars(style);
      for (const key of requiredKeys) {
        expect(typeof (chars as any)[key]).toBe("string");
      }
    }
  });
});
