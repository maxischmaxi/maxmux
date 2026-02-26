import { describe, expect, test } from "bun:test";
import { defineConfig } from "./schema.ts";

describe("defineConfig lineStyle", () => {
  test("empty config defaults lineStyle to solid", () => {
    const config = defineConfig({});
    expect(config.theme.border.lineStyle).toBe("solid");
  });

  test("lineStyle: dashed is preserved", () => {
    const config = defineConfig({
      theme: { border: { lineStyle: "dashed" } },
    });
    expect(config.theme.border.lineStyle).toBe("dashed");
  });

  test("lineStyle: dotted is preserved", () => {
    const config = defineConfig({
      theme: { border: { lineStyle: "dotted" } },
    });
    expect(config.theme.border.lineStyle).toBe("dotted");
  });

  test("lineStyle can coexist with other border options", () => {
    const config = defineConfig({
      theme: {
        border: {
          style: "sharp",
          lineStyle: "dashed",
          fg: "#ff0000",
        },
      },
    });
    expect(config.theme.border.style).toBe("sharp");
    expect(config.theme.border.lineStyle).toBe("dashed");
    expect(config.theme.border.fg).toBe("#ff0000");
  });
});
