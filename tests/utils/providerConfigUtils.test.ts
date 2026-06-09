import { describe, expect, it } from "vitest";
import type { TemplateValueConfig } from "@/config/claudeProviderPresets";
import {
  applyTemplateValues,
  extractCodexBaseUrl,
  extractCodexModelName,
  getApiKeyFromConfig,
  getCodexBaseUrl,
  hasApiKeyField,
  hasCommonConfigSnippet,
  hasTomlCommonConfigSnippet,
  isDangerousKey,
  setApiKeyInConfig,
  setCodexBaseUrl,
  setCodexModelName,
  updateCommonConfigSnippet,
  updateTomlCommonConfigSnippet,
  validateJsonConfig,
} from "@/utils/providerConfigUtils";

describe("isDangerousKey", () => {
  it("returns true for __proto__/constructor/prototype", () => {
    expect(isDangerousKey("__proto__")).toBe(true);
    expect(isDangerousKey("constructor")).toBe(true);
    expect(isDangerousKey("prototype")).toBe(true);
    expect(isDangerousKey("safe")).toBe(false);
  });
});

describe("validateJsonConfig", () => {
  it("accepts blank or object JSON", () => {
    expect(validateJsonConfig("")).toBe("");
    expect(validateJsonConfig("{}")).toBe("");
  });

  it("rejects invalid JSON or non-object values", () => {
    expect(validateJsonConfig("{bad}")).toBe("配置JSON格式错误，请检查语法");
    expect(validateJsonConfig("[]")).toBe("配置必须是 JSON 对象");
  });
});

describe("updateCommonConfigSnippet", () => {
  it("returns error on invalid config JSON", () => {
    const result = updateCommonConfigSnippet("{", "{}", true);
    expect(result.error).toBe("配置 JSON 解析失败，无法写入通用配置");
    expect(result.updatedConfig).toBe("{");
  });

  it("returns error on invalid snippet JSON", () => {
    const base = JSON.stringify({ env: { foo: "bar" } });
    const result = updateCommonConfigSnippet(base, "[]", true);
    expect(result.error).toBe("通用配置片段必须是 JSON 对象");
    expect(JSON.parse(result.updatedConfig)).toEqual({ env: { foo: "bar" } });
  });

  it("merges nested values and skips dangerous keys", () => {
    const config = {
      env: { nested: { a: 1 }, list: [1], keep: "ok" },
    };
    const snippet = {
      env: { nested: { b: 2 }, list: [2, 3], added: "yes" },
      __proto__: { polluted: "yes" },
    };

    const result = updateCommonConfigSnippet(
      JSON.stringify(config),
      JSON.stringify(snippet),
      true,
    );
    const merged = JSON.parse(result.updatedConfig);

    expect(merged).toEqual({
      env: {
        nested: { a: 1, b: 2 },
        list: [2, 3],
        keep: "ok",
        added: "yes",
      },
    });
    expect(({} as any).polluted).toBeUndefined();
    expect(Object.prototype.hasOwnProperty.call(merged, "__proto__")).toBe(
      false,
    );
  });

  it("removes only exact matches and prunes empty objects", () => {
    const config = {
      env: { nested: { a: 1, b: 2 }, arr: [1, 2], keep: "ok" },
    };
    const snippet = { env: { nested: { a: 1 }, arr: [1, 2] } };

    const result = updateCommonConfigSnippet(
      JSON.stringify(config),
      JSON.stringify(snippet),
      false,
    );

    expect(JSON.parse(result.updatedConfig)).toEqual({
      env: { nested: { b: 2 }, keep: "ok" },
    });
  });
});

describe("hasCommonConfigSnippet", () => {
  it("returns false for empty/invalid/non-object snippets", () => {
    expect(hasCommonConfigSnippet("{}", "")).toBe(false);
    expect(hasCommonConfigSnippet("{}", "not json")).toBe(false);
    expect(hasCommonConfigSnippet("{}", "[]")).toBe(false);
  });

  it("returns true for subset matches", () => {
    const config = JSON.stringify({ env: { a: 1, b: 2 } });
    const snippet = JSON.stringify({ env: { a: 1 } });
    expect(hasCommonConfigSnippet(config, snippet)).toBe(true);
  });
});

describe("API key helpers", () => {
  it("selects the right key by appType", () => {
    const config = JSON.stringify({
      env: {
        GEMINI_API_KEY: "gemini",
        CODEX_API_KEY: "codex",
        ANTHROPIC_API_KEY: "api",
        ANTHROPIC_AUTH_TOKEN: "token",
      },
    });

    expect(getApiKeyFromConfig(config, "gemini")).toBe("gemini");
    expect(getApiKeyFromConfig(config, "codex")).toBe("codex");
    expect(getApiKeyFromConfig(config)).toBe("token");
  });

  it("handles missing or invalid JSON", () => {
    expect(getApiKeyFromConfig("not json", "gemini")).toBe("");
    expect(hasApiKeyField("not json", "codex")).toBe(false);
  });

  it("detects key presence per provider", () => {
    const config = JSON.stringify({
      env: { GEMINI_API_KEY: "", CODEX_API_KEY: "", ANTHROPIC_API_KEY: "" },
    });
    expect(hasApiKeyField(config, "gemini")).toBe(true);
    expect(hasApiKeyField(config, "codex")).toBe(true);
    expect(hasApiKeyField(config)).toBe(true);
  });

  it("updates or creates keys based on options", () => {
    const geminiConfig = JSON.stringify({ env: { GEMINI_API_KEY: "old" } });
    const updatedGemini = JSON.parse(
      setApiKeyInConfig(geminiConfig, "new", { appType: "gemini" }),
    );
    expect(updatedGemini.env.GEMINI_API_KEY).toBe("new");

    const missingGemini = JSON.stringify({ env: {} });
    expect(setApiKeyInConfig(missingGemini, "new", { appType: "gemini" })).toBe(
      missingGemini,
    );
    const createdGemini = JSON.parse(
      setApiKeyInConfig(missingGemini, "new", {
        appType: "gemini",
        createIfMissing: true,
      }),
    );
    expect(createdGemini.env.GEMINI_API_KEY).toBe("new");

    const claudeConfig = JSON.stringify({
      env: { ANTHROPIC_AUTH_TOKEN: "old", ANTHROPIC_API_KEY: "api" },
    });
    const updatedClaude = JSON.parse(setApiKeyInConfig(claudeConfig, "new"));
    expect(updatedClaude.env.ANTHROPIC_AUTH_TOKEN).toBe("new");
    expect(updatedClaude.env.ANTHROPIC_API_KEY).toBe("api");

    const createClaude = JSON.parse(
      setApiKeyInConfig(JSON.stringify({ env: {} }), "created", {
        createIfMissing: true,
      }),
    );
    expect(createClaude.env.ANTHROPIC_AUTH_TOKEN).toBe("created");

    expect(setApiKeyInConfig("not json", "new")).toBe("not json");
  });
});

describe("applyTemplateValues", () => {
  it("replaces placeholders and keeps original object intact", () => {
    const templateValues: Record<string, TemplateValueConfig> = {
      HOST: {
        label: "Host",
        placeholder: "",
        defaultValue: "default",
        editorValue: "editor",
      },
      TOKEN: {
        label: "Token",
        placeholder: "",
        defaultValue: "default-token",
        editorValue: "",
      },
    };

    const config = {
      url: "https://${HOST}/v1",
      list: ["${TOKEN}", "${MISSING}", 42],
      nested: { value: "${HOST}" },
    };
    const original = JSON.parse(JSON.stringify(config));
    const result = applyTemplateValues(config, templateValues);

    expect(result).toEqual({
      url: "https://editor/v1",
      list: ["", "${MISSING}", 42],
      nested: { value: "editor" },
    });
    expect(config).toEqual(original);
  });
});

describe("TOML common config helpers", () => {
  it("returns original when snippet is empty", () => {
    const toml = "a = 1\n";
    expect(updateTomlCommonConfigSnippet(toml, "", true).updatedConfig).toBe(
      toml,
    );
  });

  it("replaces existing snippet when enabling", () => {
    const toml = 'a = 1\n\nfoo = "bar"\n\nb = 2\n';
    const result = updateTomlCommonConfigSnippet(toml, 'foo = "bar"', true);
    expect(result.updatedConfig).toBe('a = 1\nb = 2\n\nfoo = "bar"\n');
  });

  it("removes snippet when disabling", () => {
    const toml = 'a = 1\n\nfoo = "bar"\n\nb = 2\n';
    const result = updateTomlCommonConfigSnippet(toml, 'foo = "bar"', false);
    expect(result.updatedConfig).toBe("a = 1\nb = 2\n");
  });

  it("checks snippets ignoring whitespace", () => {
    expect(hasTomlCommonConfigSnippet('foo = "bar"\n', 'foo   =   "bar"')).toBe(
      true,
    );
    expect(hasTomlCommonConfigSnippet('foo = "bar"\n', "")).toBe(false);
  });
});

describe("Codex base_url helpers", () => {
  it("extracts base_url from TOML text", () => {
    expect(extractCodexBaseUrl('base_url = "https://api.example.com"\n')).toBe(
      "https://api.example.com",
    );
    expect(extractCodexBaseUrl("base_url = “https://api.example.com/”\n")).toBe(
      "https://api.example.com/",
    );
  });

  it("reads base_url from provider config", () => {
    const provider = { settingsConfig: { config: 'base_url = "https://a"' } };
    expect(getCodexBaseUrl(provider)).toBe("https://a");
  });

  it("sets or removes base_url lines", () => {
    const replaced = setCodexBaseUrl(
      'base_url = "https://old.com/"\n',
      " https://new.com/ ",
    );
    expect(replaced).toBe('base_url = "https://new.com"\n');

    const appended = setCodexBaseUrl('model = "gpt-4"\n', "https://new.com/");
    expect(appended).toBe('model = "gpt-4"\nbase_url = "https://new.com"\n');

    const removed = setCodexBaseUrl(
      'base_url = "https://old.com"\nmodel = "gpt-4"\n',
      "  ",
    );
    expect(removed).toBe('model = "gpt-4"\n');
  });
});

describe("Codex model helpers", () => {
  it("extracts model name from TOML text", () => {
    expect(extractCodexModelName('model = "gpt-4"\n')).toBe("gpt-4");
    expect(extractCodexModelName("model = 'gpt-4o'\n")).toBe("gpt-4o");
  });

  it("sets or removes model lines", () => {
    const replaced = setCodexModelName('model = "old"\n', "new");
    expect(replaced).toBe('model = "new"\n');

    const inserted = setCodexModelName(
      'model_provider = "openai"\nfoo = 1\n',
      "gpt-4",
    );
    expect(inserted).toBe(
      'model_provider = "openai"\nmodel = "gpt-4"\nfoo = 1\n',
    );

    const prepended = setCodexModelName("foo = 1\n", "gpt-4");
    expect(prepended.startsWith('model = "gpt-4"\n')).toBe(true);

    const removed = setCodexModelName('model = "old"\nfoo = 1\n', " ");
    expect(removed).toBe("foo = 1\n");
  });
});
