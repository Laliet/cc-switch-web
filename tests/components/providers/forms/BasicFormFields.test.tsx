import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useForm, FormProvider } from "react-hook-form";
import { BasicFormFields } from "@/components/providers/forms/BasicFormFields";
import type { ProviderFormData } from "@/lib/schemas/provider";

const tMock = vi.fn((key: string) => key);

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: tMock }),
}));

function TestBasicFormFields() {
  const form = useForm<ProviderFormData>({
    defaultValues: {
      name: "",
      websiteUrl: "",
      notes: "",
    },
  });

  return (
    <FormProvider {...form}>
      <BasicFormFields form={form} />
    </FormProvider>
  );
}

beforeEach(() => {
  tMock.mockClear();
});

describe("BasicFormFields", () => {
  it("renders name field with label and placeholder", () => {
    render(<TestBasicFormFields />);

    expect(screen.getByText("provider.name")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText("provider.namePlaceholder"),
    ).toBeInTheDocument();
  });

  it("renders websiteUrl field with label and placeholder", () => {
    render(<TestBasicFormFields />);

    expect(screen.getByText("provider.websiteUrl")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("https://")).toBeInTheDocument();
  });

  it("renders notes field with label and placeholder", () => {
    render(<TestBasicFormFields />);

    expect(screen.getByText("provider.notes")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText("provider.notesPlaceholder"),
    ).toBeInTheDocument();
  });

  it("allows typing in name field", async () => {
    const user = userEvent.setup();
    render(<TestBasicFormFields />);

    const nameInput = screen.getByPlaceholderText("provider.namePlaceholder");
    await user.type(nameInput, "My Provider");

    expect(nameInput).toHaveValue("My Provider");
  });

  it("allows typing in websiteUrl field", async () => {
    const user = userEvent.setup();
    render(<TestBasicFormFields />);

    const urlInput = screen.getByPlaceholderText("https://");
    await user.type(urlInput, "https://example.com");

    expect(urlInput).toHaveValue("https://example.com");
  });

  it("allows typing in notes field", async () => {
    const user = userEvent.setup();
    render(<TestBasicFormFields />);

    const notesInput = screen.getByPlaceholderText("provider.notesPlaceholder");
    await user.type(notesInput, "Some notes");

    expect(notesInput).toHaveValue("Some notes");
  });
});
