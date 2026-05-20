import { Plus, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { OpenCodeModel } from "@/types";
import {
  isKnownModelKey,
  parseModelExtraFields,
} from "./helpers/opencodeFormUtils";

interface OpenCodeFormFieldsProps {
  npm: string;
  apiKey: string;
  baseUrl: string;
  models: Record<string, OpenCodeModel>;
  extraOptions: Record<string, string>;
  onNpmChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  onBaseUrlChange: (value: string) => void;
  onModelsChange: (value: Record<string, OpenCodeModel>) => void;
  onExtraOptionsChange: (value: Record<string, string>) => void;
}

export function OpenCodeFormFields({
  npm,
  apiKey,
  baseUrl,
  models,
  extraOptions,
  onNpmChange,
  onApiKeyChange,
  onBaseUrlChange,
  onModelsChange,
  onExtraOptionsChange,
}: OpenCodeFormFieldsProps) {
  const { t } = useTranslation();
  const modelEntries = Object.entries(models);
  const extraOptionEntries = Object.entries(extraOptions);
  const [expandedModelIds, setExpandedModelIds] = useState<Set<string>>(
    () => new Set(),
  );

  const updateModelId = (oldId: string, nextId: string) => {
    const id = nextId.trim();
    if (!id || id === oldId) return;
    const next: Record<string, OpenCodeModel> = {};
    for (const [key, value] of Object.entries(models)) {
      next[key === oldId ? id : key] = value;
    }
    onModelsChange(next);
  };

  const updateModelName = (id: string, name: string) => {
    onModelsChange({
      ...models,
      [id]: {
        ...models[id],
        name,
      },
    });
  };

  const updateModelLimit = (
    id: string,
    key: "context" | "output",
    rawValue: string,
  ) => {
    const nextLimit = { ...(models[id]?.limit ?? {}) };
    const trimmed = rawValue.trim();
    if (!trimmed) {
      delete nextLimit[key];
    } else {
      const numericValue = Number(trimmed);
      if (!Number.isFinite(numericValue) || numericValue < 0) return;
      nextLimit[key] = numericValue;
    }

    onModelsChange({
      ...models,
      [id]: {
        ...models[id],
        limit: Object.keys(nextLimit).length > 0 ? nextLimit : undefined,
      },
    });
  };

  const updateModelOptionKey = (
    id: string,
    oldKey: string,
    nextKey: string,
  ) => {
    const key = nextKey.trim();
    if (!key || key === oldKey) return;
    const nextOptions: Record<string, unknown> = {};
    for (const [currentKey, value] of Object.entries(
      models[id]?.options ?? {},
    )) {
      nextOptions[currentKey === oldKey ? key : currentKey] = value;
    }
    onModelsChange({
      ...models,
      [id]: {
        ...models[id],
        options: nextOptions,
      },
    });
  };

  const updateModelOptionValue = (id: string, key: string, value: string) => {
    const nextOptions = { ...(models[id]?.options ?? {}) };
    try {
      nextOptions[key] = JSON.parse(value);
    } catch {
      nextOptions[key] = value;
    }
    onModelsChange({
      ...models,
      [id]: {
        ...models[id],
        options: nextOptions,
      },
    });
  };

  const addModelOption = (id: string) => {
    onModelsChange({
      ...models,
      [id]: {
        ...models[id],
        options: {
          ...(models[id]?.options ?? {}),
          [`option-${Date.now()}`]: "",
        },
      },
    });
  };

  const removeModelOption = (id: string, key: string) => {
    const nextOptions = { ...(models[id]?.options ?? {}) };
    delete nextOptions[key];
    onModelsChange({
      ...models,
      [id]: {
        ...models[id],
        options: Object.keys(nextOptions).length > 0 ? nextOptions : undefined,
      },
    });
  };

  const updateModelExtraKey = (id: string, oldKey: string, nextKey: string) => {
    const key = nextKey.trim();
    if (!key || key === oldKey || isKnownModelKey(key)) return;
    const currentModel = models[id] ?? {};
    const nextModel: OpenCodeModel = {};
    for (const [currentKey, value] of Object.entries(currentModel)) {
      nextModel[currentKey === oldKey ? key : currentKey] = value;
    }
    onModelsChange({
      ...models,
      [id]: nextModel,
    });
  };

  const updateModelExtraValue = (id: string, key: string, value: string) => {
    const currentModel = models[id] ?? {};
    try {
      onModelsChange({
        ...models,
        [id]: {
          ...currentModel,
          [key]: JSON.parse(value),
        },
      });
    } catch {
      onModelsChange({
        ...models,
        [id]: {
          ...currentModel,
          [key]: value,
        },
      });
    }
  };

  const addModelExtraField = (id: string) => {
    onModelsChange({
      ...models,
      [id]: {
        ...models[id],
        [`field-${Date.now()}`]: "",
      },
    });
  };

  const removeModelExtraField = (id: string, key: string) => {
    const nextModel = { ...(models[id] ?? {}) };
    delete nextModel[key];
    onModelsChange({
      ...models,
      [id]: nextModel,
    });
  };

  const toggleModelExpanded = (id: string) => {
    setExpandedModelIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const addModel = () => {
    const id = `model-${Date.now()}`;
    onModelsChange({
      ...models,
      [id]: { name: "" },
    });
  };

  const removeModel = (id: string) => {
    const next = { ...models };
    delete next[id];
    onModelsChange(next);
  };

  const updateOptionKey = (oldKey: string, nextKey: string) => {
    const key = nextKey.trim();
    if (!key || key === oldKey) return;
    const next: Record<string, string> = {};
    for (const [currentKey, value] of Object.entries(extraOptions)) {
      next[currentKey === oldKey ? key : currentKey] = value;
    }
    onExtraOptionsChange(next);
  };

  const updateOptionValue = (key: string, value: string) => {
    onExtraOptionsChange({
      ...extraOptions,
      [key]: value,
    });
  };

  const addOption = () => {
    onExtraOptionsChange({
      ...extraOptions,
      [`option-${Date.now()}`]: "",
    });
  };

  const removeOption = (key: string) => {
    const next = { ...extraOptions };
    delete next[key];
    onExtraOptionsChange(next);
  };

  return (
    <div className="space-y-4 rounded-lg border border-border-default p-4">
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="space-y-2">
          <Label htmlFor="opencode-npm">
            {t("providerForm.opencodeNpm", {
              defaultValue: "OpenCode NPM 包",
            })}
          </Label>
          <Input
            id="opencode-npm"
            value={npm}
            onChange={(event) => onNpmChange(event.target.value)}
            placeholder="@ai-sdk/openai-compatible"
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="opencode-base-url">
            {t("providerForm.baseUrl", { defaultValue: "请求地址" })}
          </Label>
          <Input
            id="opencode-base-url"
            value={baseUrl}
            onChange={(event) => onBaseUrlChange(event.target.value)}
            placeholder="https://api.example.com/v1"
          />
        </div>
      </div>

      <div className="space-y-2">
        <Label htmlFor="opencode-api-key">
          {t("providerForm.apiKey", { defaultValue: "API Key" })}
        </Label>
        <Input
          id="opencode-api-key"
          value={apiKey}
          onChange={(event) => onApiKeyChange(event.target.value)}
          placeholder="sk-..."
          type="password"
          autoComplete="off"
        />
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between gap-2">
          <Label>
            {t("providerForm.opencodeModels", {
              defaultValue: "OpenCode 模型",
            })}
          </Label>
          <Button type="button" variant="outline" size="sm" onClick={addModel}>
            <Plus className="h-4 w-4" />
            {t("common.add", { defaultValue: "添加" })}
          </Button>
        </div>
        <div className="space-y-2">
          {modelEntries.length === 0 ? (
            <p className="text-xs text-muted-foreground">
              {t("providerForm.opencodeModelsEmpty", {
                defaultValue:
                  "未指定模型时 OpenCode 会使用 provider 默认模型。",
              })}
            </p>
          ) : (
            modelEntries.map(([id, model]) => (
              <OpenCodeModelEditor
                key={id}
                id={id}
                model={model}
                expanded={expandedModelIds.has(id)}
                onToggleExpanded={() => toggleModelExpanded(id)}
                onUpdateId={updateModelId}
                onUpdateName={updateModelName}
                onUpdateLimit={updateModelLimit}
                onAddOption={addModelOption}
                onUpdateOptionKey={updateModelOptionKey}
                onUpdateOptionValue={updateModelOptionValue}
                onRemoveOption={removeModelOption}
                onAddExtraField={addModelExtraField}
                onUpdateExtraKey={updateModelExtraKey}
                onUpdateExtraValue={updateModelExtraValue}
                onRemoveExtraField={removeModelExtraField}
                onRemove={removeModel}
              />
            ))
          )}
        </div>
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between gap-2">
          <Label>
            {t("providerForm.opencodeExtraOptions", {
              defaultValue: "额外 options",
            })}
          </Label>
          <Button type="button" variant="outline" size="sm" onClick={addOption}>
            <Plus className="h-4 w-4" />
            {t("common.add", { defaultValue: "添加" })}
          </Button>
        </div>
        <div className="space-y-2">
          {extraOptionEntries.map(([key, value]) => (
            <div key={key} className="grid gap-2 sm:grid-cols-[1fr_1fr_auto]">
              <Input
                defaultValue={key.startsWith("option-") ? "" : key}
                onBlur={(event) => updateOptionKey(key, event.target.value)}
                placeholder="headers"
              />
              <Input
                value={value}
                onChange={(event) => updateOptionValue(key, event.target.value)}
                placeholder='{"X-Provider":"example"}'
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                onClick={() => removeOption(key)}
                aria-label={t("common.delete", { defaultValue: "删除" })}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

interface OpenCodeModelEditorProps {
  id: string;
  model: OpenCodeModel;
  expanded: boolean;
  onToggleExpanded: () => void;
  onUpdateId: (oldId: string, nextId: string) => void;
  onUpdateName: (id: string, name: string) => void;
  onUpdateLimit: (
    id: string,
    key: "context" | "output",
    rawValue: string,
  ) => void;
  onAddOption: (id: string) => void;
  onUpdateOptionKey: (id: string, oldKey: string, nextKey: string) => void;
  onUpdateOptionValue: (id: string, key: string, value: string) => void;
  onRemoveOption: (id: string, key: string) => void;
  onAddExtraField: (id: string) => void;
  onUpdateExtraKey: (id: string, oldKey: string, nextKey: string) => void;
  onUpdateExtraValue: (id: string, key: string, value: string) => void;
  onRemoveExtraField: (id: string, key: string) => void;
  onRemove: (id: string) => void;
}

function stringifyFieldValue(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value);
}

function OpenCodeModelEditor({
  id,
  model,
  expanded,
  onToggleExpanded,
  onUpdateId,
  onUpdateName,
  onUpdateLimit,
  onAddOption,
  onUpdateOptionKey,
  onUpdateOptionValue,
  onRemoveOption,
  onAddExtraField,
  onUpdateExtraKey,
  onUpdateExtraValue,
  onRemoveExtraField,
  onRemove,
}: OpenCodeModelEditorProps) {
  const { t } = useTranslation();
  const optionEntries = useMemo(
    () => Object.entries(model.options ?? {}),
    [model.options],
  );
  const extraFieldEntries = useMemo(
    () => Object.entries(parseModelExtraFields(model)),
    [model],
  );

  return (
    <div className="space-y-2 rounded-md border border-border-default p-3">
      <div className="grid gap-2 sm:grid-cols-[1fr_1fr_auto_auto]">
        <Input
          defaultValue={id}
          onBlur={(event) => onUpdateId(id, event.target.value)}
          placeholder="gpt-5-codex"
        />
        <Input
          value={typeof model.name === "string" ? model.name : ""}
          onChange={(event) => onUpdateName(id, event.target.value)}
          placeholder={t("providerForm.displayName", {
            defaultValue: "显示名称",
          })}
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={onToggleExpanded}
        >
          {expanded
            ? t("common.collapse", { defaultValue: "收起" })
            : t("common.expand", { defaultValue: "展开" })}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          onClick={() => onRemove(id)}
          aria-label={t("common.delete", { defaultValue: "删除" })}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>

      {expanded && (
        <div className="space-y-4 border-t border-border-default pt-3">
          <div className="space-y-2">
            <Label>
              {t("providerForm.opencodeModelLimit", {
                defaultValue: "模型限额",
              })}
            </Label>
            <div className="grid gap-2 sm:grid-cols-2">
              <Input
                type="number"
                min={0}
                value={
                  typeof model.limit?.context === "number"
                    ? String(model.limit.context)
                    : ""
                }
                onChange={(event) =>
                  onUpdateLimit(id, "context", event.target.value)
                }
                placeholder="context"
              />
              <Input
                type="number"
                min={0}
                value={
                  typeof model.limit?.output === "number"
                    ? String(model.limit.output)
                    : ""
                }
                onChange={(event) =>
                  onUpdateLimit(id, "output", event.target.value)
                }
                placeholder="output"
              />
            </div>
          </div>

          <KeyValueEditor
            title={t("providerForm.opencodeModelOptions", {
              defaultValue: "模型 options",
            })}
            entries={optionEntries.map(([key, value]) => [
              key,
              stringifyFieldValue(value),
            ])}
            keyPlaceholder="reasoningEffort"
            valuePlaceholder='"high"'
            addLabel={t("common.add", { defaultValue: "添加" })}
            emptyText={t("providerForm.opencodeModelOptionsEmpty", {
              defaultValue: "未配置模型专属 options。",
            })}
            onAdd={() => onAddOption(id)}
            onUpdateKey={(oldKey, nextKey) =>
              onUpdateOptionKey(id, oldKey, nextKey)
            }
            onUpdateValue={(key, value) => onUpdateOptionValue(id, key, value)}
            onRemove={(key) => onRemoveOption(id, key)}
          />

          <KeyValueEditor
            title={t("providerForm.opencodeModelExtraFields", {
              defaultValue: "模型扩展字段",
            })}
            entries={extraFieldEntries}
            keyPlaceholder="releaseDate"
            valuePlaceholder='"2026-01-01"'
            addLabel={t("common.add", { defaultValue: "添加" })}
            emptyText={t("providerForm.opencodeModelExtraFieldsEmpty", {
              defaultValue: "未配置模型扩展字段。",
            })}
            onAdd={() => onAddExtraField(id)}
            onUpdateKey={(oldKey, nextKey) =>
              onUpdateExtraKey(id, oldKey, nextKey)
            }
            onUpdateValue={(key, value) => onUpdateExtraValue(id, key, value)}
            onRemove={(key) => onRemoveExtraField(id, key)}
          />
        </div>
      )}
    </div>
  );
}

interface KeyValueEditorProps {
  title: string;
  entries: Array<[string, string]>;
  keyPlaceholder: string;
  valuePlaceholder: string;
  addLabel: string;
  emptyText: string;
  onAdd: () => void;
  onUpdateKey: (oldKey: string, nextKey: string) => void;
  onUpdateValue: (key: string, value: string) => void;
  onRemove: (key: string) => void;
}

function KeyValueEditor({
  title,
  entries,
  keyPlaceholder,
  valuePlaceholder,
  addLabel,
  emptyText,
  onAdd,
  onUpdateKey,
  onUpdateValue,
  onRemove,
}: KeyValueEditorProps) {
  const { t } = useTranslation();

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-2">
        <Label>{title}</Label>
        <Button type="button" variant="outline" size="sm" onClick={onAdd}>
          <Plus className="h-4 w-4" />
          {addLabel}
        </Button>
      </div>
      {entries.length === 0 ? (
        <p className="text-xs text-muted-foreground">{emptyText}</p>
      ) : (
        <div className="space-y-2">
          {entries.map(([key, value]) => (
            <div key={key} className="grid gap-2 sm:grid-cols-[1fr_1fr_auto]">
              <Input
                defaultValue={
                  key.startsWith("option-") || key.startsWith("field-")
                    ? ""
                    : key
                }
                onBlur={(event) => onUpdateKey(key, event.target.value)}
                placeholder={keyPlaceholder}
              />
              <Input
                value={value}
                onChange={(event) => onUpdateValue(key, event.target.value)}
                placeholder={valuePlaceholder}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                onClick={() => onRemove(key)}
                aria-label={t("common.delete", { defaultValue: "删除" })}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
