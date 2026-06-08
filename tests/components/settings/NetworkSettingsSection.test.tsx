import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { NetworkSettingsSection } from "@/components/settings/NetworkSettingsSection";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) =>
      options?.defaultValue ?? _key,
  }),
}));

describe("NetworkSettingsSection", () => {
  it("updates the GitHub mirror URL", () => {
    const onChange = vi.fn();
    render(
      <NetworkSettingsSection
        value={{ githubMirrorBaseUrl: "" }}
        onChange={onChange}
      />,
    );

    fireEvent.change(screen.getByLabelText("GitHub mirror base URL"), {
      target: { value: "https://ghproxy.net/" },
    });

    expect(onChange).toHaveBeenCalledWith({
      githubMirrorBaseUrl: "https://ghproxy.net/",
    });
  });

  it("resets to GitHub origin", () => {
    const onChange = vi.fn();
    render(
      <NetworkSettingsSection
        value={{ githubMirrorBaseUrl: "https://ghproxy.net/" }}
        onChange={onChange}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Use GitHub/i }));

    expect(onChange).toHaveBeenCalledWith({
      githubMirrorBaseUrl: "",
    });
  });
});
