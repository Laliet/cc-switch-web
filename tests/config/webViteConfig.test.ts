import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

const configSource = readFileSync(
  path.resolve(__dirname, "../../vite.config.web.mts"),
  "utf8",
);

describe("web Vite manual chunks", () => {
  it("keeps react-i18next and i18next in the react vendor chunk", () => {
    const vendorReactBlock = configSource.match(
      /if \(\s*([\s\S]*?)\s*\) \{\s*return "vendor-react";\s*\}/,
    )?.[1];

    expect(vendorReactBlock).toContain('id.includes("/react/")');
    expect(vendorReactBlock).toContain('id.includes("i18next")');
    expect(vendorReactBlock).toContain('id.includes("react-i18next")');
  });

  it("does not emit a separate i18n vendor chunk", () => {
    expect(configSource).not.toContain('return "vendor-i18n"');
  });
});
