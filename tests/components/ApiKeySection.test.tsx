import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ApiKeySection } from "@/components/providers/forms/shared";

describe("ApiKeySection websiteUrl safety", () => {
  it("renders an external link only for http/https", () => {
    const { rerender } = render(
      <ApiKeySection
        value="test"
        onChange={() => {}}
        shouldShowLink
        websiteUrl=" https://example.com/get-key "
      />,
    );

    expect(screen.getByRole("link", { name: /获取 API Key/i })).toHaveAttribute(
      "href",
      "https://example.com/get-key",
    );

    rerender(
      <ApiKeySection
        value="test"
        onChange={() => {}}
        shouldShowLink
        websiteUrl="javascript:alert(1)"
      />,
    );

    expect(screen.queryByRole("link", { name: /获取 API Key/i })).toBeNull();
  });
});
