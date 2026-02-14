import type { TFunction } from "i18next";
import { describe, expect, it, vi } from "vitest";
import { formatSkillError, parseSkillError } from "@/lib/errors/skillErrorParser";

const createT = (): TFunction =>
  vi.fn((key: string, context?: Record<string, string>) => {
    if (context && Object.keys(context).length > 0) {
      return `${key}:${JSON.stringify(context)}`;
    }
    if (context) {
      return `${key}:{}`;
    }
    return key;
  }) as unknown as TFunction;

describe("parseSkillError", () => {
  it("returns structured error for JSON string code", () => {
    const input = JSON.stringify("SKILL_NOT_FOUND");

    expect(parseSkillError(input)).toEqual({
      code: "SKILL_NOT_FOUND",
      context: {},
    });
  });

  it("returns structured error for JSON object code", () => {
    const input = JSON.stringify({ code: "DOWNLOAD_FAILED" });

    expect(parseSkillError(input)).toEqual({
      code: "DOWNLOAD_FAILED",
      context: {},
      suggestion: undefined,
    });
  });

  it("normalizes context values", () => {
    const input = JSON.stringify({
      code: "MISSING_REPO_INFO",
      context: {
        repo: "alpha",
        retries: 2,
        active: true,
        missing: null,
      },
    });

    expect(parseSkillError(input)).toEqual({
      code: "MISSING_REPO_INFO",
      context: {
        repo: "alpha",
        retries: "2",
        active: "true",
      },
      suggestion: undefined,
    });
  });

  it("parses suggestion field", () => {
    const input = JSON.stringify({
      code: "DOWNLOAD_FAILED",
      context: { reason: "timeout" },
      suggestion: "checkNetwork",
    });

    expect(parseSkillError(input)).toEqual({
      code: "DOWNLOAD_FAILED",
      context: { reason: "timeout" },
      suggestion: "checkNetwork",
    });
  });

  it("returns structured error for non-JSON known code", () => {
    expect(parseSkillError("DOWNLOAD_TIMEOUT")).toEqual({
      code: "DOWNLOAD_TIMEOUT",
      context: {},
    });
  });

  it("extracts app id from localized not-supported message", () => {
    expect(parseSkillError("应用 'opencode' 暂未支持，敬请期待。")).toEqual({
      code: "APP_NOT_SUPPORTED",
      context: { app: "opencode" },
    });
  });

  it("returns null for unknown format", () => {
    expect(parseSkillError("unknown error")).toBeNull();
  });
});

describe("formatSkillError", () => {
  it("returns translated title and description for structured error", () => {
    const t = createT();
    const input = JSON.stringify({
      code: "SKILL_NOT_FOUND",
      context: { name: "demo" },
    });

    expect(formatSkillError(input, t)).toEqual({
      title: "skills.installFailed",
      description: "skills.error.skillNotFound:{\"name\":\"demo\"}",
    });
  });

  it("formats not-supported app error from plain message", () => {
    const t = createT();

    expect(formatSkillError("App 'omo' is not supported yet.", t)).toEqual({
      title: "skills.installFailed",
      description: "skills.error.appNotSupported:{\"app\":\"omo\"}",
    });
  });

  it("returns original string for non-structured error", () => {
    const t = createT();

    expect(formatSkillError("Something blew up", t)).toEqual({
      title: "skills.installFailed",
      description: "Something blew up",
    });
  });

  it("appends suggestion text when present", () => {
    const t = createT();
    const input = JSON.stringify({
      code: "DOWNLOAD_FAILED",
      context: { reason: "offline" },
      suggestion: "checkNetwork",
    });

    expect(formatSkillError(input, t)).toEqual({
      title: "skills.installFailed",
      description:
        "skills.error.downloadFailed:{\"reason\":\"offline\"}\n\nskills.error.suggestion.checkNetwork",
    });
  });

  it("returns default error message for empty string", () => {
    const t = createT();

    expect(formatSkillError("", t)).toEqual({
      title: "skills.installFailed",
      description: "common.error",
    });
  });
});
