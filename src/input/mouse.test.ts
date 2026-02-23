import { describe, expect, test } from "bun:test";
import {
  parseSgrMouse,
  encodeSgrMouse,
  getBaseButton,
  isScrollEvent,
  isMotionEvent,
  MOUSE_LEFT,
  MOUSE_MIDDLE,
  MOUSE_RIGHT,
  MOUSE_SCROLL_UP,
  MOUSE_SCROLL_DOWN,
} from "./mouse.ts";

describe("parseSgrMouse", () => {
  test("parses left click", () => {
    // \x1b[<0;5;10M — left press at col 5, row 10 (1-based)
    const buf = Buffer.from("\x1b[<0;5;10M");
    const result = parseSgrMouse(buf);
    expect(result).not.toBeNull();
    expect(result!.event.button).toBe(0);
    expect(result!.event.x).toBe(4); // 0-based
    expect(result!.event.y).toBe(9); // 0-based
    expect(result!.event.isRelease).toBe(false);
    expect(result!.consumed).toBe(buf.length);
  });

  test("parses left release", () => {
    const buf = Buffer.from("\x1b[<0;5;10m");
    const result = parseSgrMouse(buf);
    expect(result).not.toBeNull();
    expect(result!.event.isRelease).toBe(true);
    expect(result!.event.button).toBe(0);
  });

  test("parses right click", () => {
    const buf = Buffer.from("\x1b[<2;1;1M");
    const result = parseSgrMouse(buf);
    expect(result).not.toBeNull();
    expect(result!.event.button).toBe(2);
    expect(result!.event.x).toBe(0);
    expect(result!.event.y).toBe(0);
  });

  test("parses middle click", () => {
    const buf = Buffer.from("\x1b[<1;20;30M");
    const result = parseSgrMouse(buf);
    expect(result).not.toBeNull();
    expect(result!.event.button).toBe(1);
    expect(result!.event.x).toBe(19);
    expect(result!.event.y).toBe(29);
  });

  test("parses scroll up", () => {
    const buf = Buffer.from("\x1b[<64;10;20M");
    const result = parseSgrMouse(buf);
    expect(result).not.toBeNull();
    expect(result!.event.button).toBe(64);
  });

  test("parses scroll down", () => {
    const buf = Buffer.from("\x1b[<65;10;20M");
    const result = parseSgrMouse(buf);
    expect(result).not.toBeNull();
    expect(result!.event.button).toBe(65);
  });

  test("parses motion event (drag)", () => {
    // Button 32 = motion flag + left button
    const buf = Buffer.from("\x1b[<32;15;25M");
    const result = parseSgrMouse(buf);
    expect(result).not.toBeNull();
    expect(result!.event.button).toBe(32);
    expect(isMotionEvent(result!.event.button)).toBe(true);
  });

  test("parses large coordinates", () => {
    const buf = Buffer.from("\x1b[<0;200;100M");
    const result = parseSgrMouse(buf);
    expect(result).not.toBeNull();
    expect(result!.event.x).toBe(199);
    expect(result!.event.y).toBe(99);
  });

  test("returns null for incomplete sequence", () => {
    const buf = Buffer.from("\x1b[<0;5;");
    const result = parseSgrMouse(buf);
    expect(result).toBeNull();
  });

  test("returns null for non-mouse escape", () => {
    const buf = Buffer.from("\x1b[A"); // Arrow up
    const result = parseSgrMouse(buf);
    expect(result).toBeNull();
  });

  test("returns null for too-short buffer", () => {
    const buf = Buffer.from("\x1b[<");
    const result = parseSgrMouse(buf);
    expect(result).toBeNull();
  });

  test("parses with offset", () => {
    const buf = Buffer.from("abc\x1b[<0;3;4M");
    const result = parseSgrMouse(buf, 3);
    expect(result).not.toBeNull();
    expect(result!.event.x).toBe(2);
    expect(result!.event.y).toBe(3);
    expect(result!.consumed).toBe(buf.length - 3);
  });

  test("parses multiple events in sequence", () => {
    const buf = Buffer.from("\x1b[<0;1;1M\x1b[<0;1;1m");
    const first = parseSgrMouse(buf, 0);
    expect(first).not.toBeNull();
    expect(first!.event.isRelease).toBe(false);

    const second = parseSgrMouse(buf, first!.consumed);
    expect(second).not.toBeNull();
    expect(second!.event.isRelease).toBe(true);
  });

  test("returns null for invalid characters in sequence", () => {
    const buf = Buffer.from("\x1b[<0;x;1M");
    const result = parseSgrMouse(buf);
    expect(result).toBeNull();
  });
});

describe("encodeSgrMouse", () => {
  test("encodes left press", () => {
    const encoded = encodeSgrMouse(0, 4, 9, false);
    expect(encoded).toBe("\x1b[<0;5;10M");
  });

  test("encodes left release", () => {
    const encoded = encodeSgrMouse(0, 4, 9, true);
    expect(encoded).toBe("\x1b[<0;5;10m");
  });

  test("encodes scroll", () => {
    const encoded = encodeSgrMouse(64, 0, 0, false);
    expect(encoded).toBe("\x1b[<64;1;1M");
  });

  test("encode/decode roundtrip", () => {
    const original = { button: 2, x: 50, y: 30, isRelease: false };
    const encoded = encodeSgrMouse(
      original.button,
      original.x,
      original.y,
      original.isRelease,
    );
    const parsed = parseSgrMouse(Buffer.from(encoded));
    expect(parsed).not.toBeNull();
    expect(parsed!.event.button).toBe(original.button);
    expect(parsed!.event.x).toBe(original.x);
    expect(parsed!.event.y).toBe(original.y);
    expect(parsed!.event.isRelease).toBe(original.isRelease);
  });
});

describe("helper functions", () => {
  test("getBaseButton extracts button from combined value", () => {
    expect(getBaseButton(MOUSE_LEFT)).toBe(MOUSE_LEFT);
    expect(getBaseButton(MOUSE_MIDDLE)).toBe(MOUSE_MIDDLE);
    expect(getBaseButton(MOUSE_RIGHT)).toBe(MOUSE_RIGHT);
    expect(getBaseButton(MOUSE_SCROLL_UP)).toBe(MOUSE_SCROLL_UP);
    expect(getBaseButton(MOUSE_SCROLL_DOWN)).toBe(MOUSE_SCROLL_DOWN);
    // With shift modifier (bit 2)
    expect(getBaseButton(4)).toBe(0); // shift + left = still left base
    // With ctrl modifier (bit 4)
    expect(getBaseButton(16)).toBe(0); // ctrl + left = still left base
    // Motion + left (bit 5 = motion flag, masked out)
    expect(getBaseButton(32)).toBe(MOUSE_LEFT); // motion stripped, base is left
  });

  test("isScrollEvent detects scroll", () => {
    expect(isScrollEvent(MOUSE_SCROLL_UP)).toBe(true);
    expect(isScrollEvent(MOUSE_SCROLL_DOWN)).toBe(true);
    expect(isScrollEvent(MOUSE_LEFT)).toBe(false);
    expect(isScrollEvent(MOUSE_RIGHT)).toBe(false);
  });

  test("isMotionEvent detects motion", () => {
    expect(isMotionEvent(32)).toBe(true); // motion + left
    expect(isMotionEvent(33)).toBe(true); // motion + middle
    expect(isMotionEvent(0)).toBe(false);
    expect(isMotionEvent(2)).toBe(false);
  });
});
