import { useState, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import {
  Plus,
  Trash2,
  ChevronDown,
  ChevronRight,
  Wand2,
  Settings,
  FolderInput,
  Loader2,
  HelpCircle,
  Check,
  ChevronsUpDown,
  X,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { toast } from "sonner";
import { useReadOmoLocalFile, useReadOmoSlimLocalFile } from "@/lib/query/omo";
import {
  OMO_BUILTIN_AGENTS,
  OMO_BUILTIN_CATEGORIES,
  OMO_BABYSITTING_PLACEHOLDER,
  OMO_BACKGROUND_TASK_PLACEHOLDER,
  OMO_BROWSER_AUTOMATION_PLACEHOLDER,
  OMO_CLAUDE_CODE_PLACEHOLDER,
  OMO_COMMENT_CHECKER_PLACEHOLDER,
  OMO_DEFAULT_SCHEMA_URL,
  OMO_DISABLEABLE_AGENTS,
  OMO_DISABLEABLE_COMMANDS,
  OMO_DISABLEABLE_HOOKS,
  OMO_DISABLEABLE_MCPS,
  OMO_DISABLEABLE_SKILLS,
  OMO_DISABLEABLE_TOOLS,
  OMO_EXPERIMENTAL_PLACEHOLDER,
  OMO_GIT_MASTER_PLACEHOLDER,
  OMO_MODEL_CAPABILITIES_PLACEHOLDER,
  OMO_OPENCLAW_PLACEHOLDER,
  OMO_RALPH_LOOP_PLACEHOLDER,
  OMO_RUNTIME_FALLBACK_PLACEHOLDER,
  OMO_SISYPHUS_AGENT_PLACEHOLDER,
  OMO_SLIM_BUILTIN_AGENTS,
  OMO_SLIM_COUNCIL_PLACEHOLDER,
  OMO_SLIM_DEFAULT_SCHEMA_URL,
  OMO_SLIM_DISABLEABLE_AGENTS,
  OMO_SLIM_DISABLEABLE_HOOKS,
  OMO_SLIM_DISABLEABLE_MCPS,
  OMO_SLIM_FALLBACK_PLACEHOLDER,
  OMO_SLIM_MULTIPLEXER_PLACEHOLDER,
  OMO_SLIM_TODO_CONTINUATION_PLACEHOLDER,
  OMO_START_WORK_PLACEHOLDER,
  OMO_TMUX_PLACEHOLDER,
  OMO_WEBSEARCH_PLACEHOLDER,
  type OmoAgentDef,
  type OmoCategoryDef,
  parseOmoOtherFieldsObject,
} from "@/types/omo";

const ADVANCED_PLACEHOLDER = `{
  "temperature": 0.5,
  "top_p": 0.9,
  "budgetTokens": 20000,
  "prompt_append": "",
  "permission": { "edit": "allow", "bash": "ask" }
}`;

interface OmoFormFieldsProps {
  modelOptions: Array<{ value: string; label: string }>;
  modelVariantsMap?: Record<string, string[]>;
  presetMetaMap?: Record<
    string,
    {
      options?: Record<string, unknown>;
      limit?: { context?: number; output?: number };
    }
  >;
  agents: Record<string, Record<string, unknown>>;
  onAgentsChange: (agents: Record<string, Record<string, unknown>>) => void;
  categories?: Record<string, Record<string, unknown>>;
  onCategoriesChange?: (
    categories: Record<string, Record<string, unknown>>,
  ) => void;
  otherFieldsStr: string;
  onOtherFieldsStrChange: (value: string) => void;
  isSlim?: boolean;
}

export type CustomModelItem = {
  key: string;
  model: string;
  sourceKey?: string;
};
type BuiltinModelDef = Pick<
  OmoAgentDef | OmoCategoryDef,
  "key" | "display" | "descKey" | "recommended" | "tooltipKey"
>;
type ModelOption = { value: string; label: string };
type PresetModelMeta = {
  options?: Record<string, unknown>;
  limit?: { context?: number; output?: number };
};
type SelectableOption = { value: string; label: string };
const OTHER_FIELDS_INDENT = 2;

function DeferredKeyInput({
  value,
  onCommit,
  placeholder,
  className,
}: {
  value: string;
  onCommit: (value: string) => void;
  placeholder?: string;
  className?: string;
}) {
  const [draft, setDraft] = useState(value);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  return (
    <Input
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        if (draft !== value) {
          onCommit(draft);
        }
      }}
      placeholder={placeholder}
      className={className}
    />
  );
}

const BUILTIN_AGENT_KEYS = new Set(OMO_BUILTIN_AGENTS.map((a) => a.key));
const BUILTIN_AGENT_KEYS_SLIM = new Set(
  OMO_SLIM_BUILTIN_AGENTS.map((a) => a.key),
);
const BUILTIN_CATEGORY_KEYS = new Set(OMO_BUILTIN_CATEGORIES.map((c) => c.key));
const EMPTY_VARIANT_VALUE = "__cc_switch_omo_variant_empty__";

function ModelCombobox({
  value,
  options,
  recommended,
  onChange,
}: {
  value: string;
  options: ModelOption[];
  recommended?: string;
  onChange: (value: string) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  const selectedLabel = options.find((o) => o.value === value)?.label;

  const selectModelText = t("omo.selectModel", {
    defaultValue: "Select configured model",
  });
  const placeholderText = recommended
    ? `${selectModelText} (${t("omo.recommendedHint", { model: recommended, defaultValue: "Recommended: {{model}}" })})`
    : selectModelText;

  return (
    <Popover modal open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          role="combobox"
          aria-expanded={open}
          className="flex flex-1 h-8 items-center justify-between whitespace-nowrap rounded-md border border-border-default bg-background px-3 py-1 text-sm shadow-sm ring-offset-background focus:outline-none focus-visible:outline-none focus:border-border-default focus-visible:border-border-default focus:ring-0 focus-visible:ring-0 disabled:cursor-not-allowed disabled:opacity-50"
        >
          <span className={cn("truncate", !value && "text-muted-foreground")}>
            {selectedLabel || placeholderText}
          </span>
          <span className="flex items-center shrink-0 ml-1 gap-0.5">
            {value && (
              <X
                className="h-3.5 w-3.5 opacity-50 hover:opacity-100 cursor-pointer"
                onClick={(e) => {
                  e.stopPropagation();
                  onChange("");
                }}
              />
            )}
            <ChevronsUpDown className="h-3.5 w-3.5 opacity-50" />
          </span>
        </button>
      </PopoverTrigger>
      <PopoverContent
        side="bottom"
        align="start"
        sideOffset={6}
        avoidCollisions={true}
        collisionPadding={8}
        className="z-[1000] w-[var(--radix-popover-trigger-width)] p-0 border-border-default"
      >
        <Command>
          <CommandInput
            placeholder={t("omo.searchModel", {
              defaultValue: "Search model...",
            })}
          />
          <CommandList>
            <CommandEmpty>
              {t("omo.noEnabledModels", {
                defaultValue: "No configured models",
              })}
            </CommandEmpty>
            <CommandGroup>
              {options.map((option) => (
                <CommandItem
                  key={option.value}
                  value={option.value}
                  keywords={[option.label]}
                  onSelect={() => {
                    onChange(option.value);
                    setOpen(false);
                  }}
                >
                  <Check
                    className={cn(
                      "mr-2 h-4 w-4",
                      value === option.value ? "opacity-100" : "opacity-0",
                    )}
                  />
                  {option.label}
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}

function getAdvancedStr(config: Record<string, unknown> | undefined): string {
  if (!config) return "";
  const adv: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(config)) {
    if (k !== "model" && k !== "variant") adv[k] = v;
  }
  return Object.keys(adv).length > 0 ? JSON.stringify(adv, null, 2) : "";
}

function mergePresetMetaIntoEntry(
  entry: Record<string, unknown>,
  model: string,
  presetMetaMap: Record<string, PresetModelMeta>,
): Record<string, unknown> {
  const meta = presetMetaMap[model];
  if (!meta) return entry;

  return {
    ...entry,
    ...(meta.limit && entry.limit === undefined ? { limit: meta.limit } : {}),
    ...(meta.options && entry.options === undefined
      ? { options: meta.options }
      : {}),
  };
}

function parseOtherFieldsForEdit(raw: string): Record<string, unknown> {
  try {
    return parseOmoOtherFieldsObject(raw) ?? {};
  } catch {
    return {};
  }
}

function stringifyOtherFieldsForEdit(fields: Record<string, unknown>): string {
  return Object.keys(fields).length > 0
    ? JSON.stringify(fields, null, OTHER_FIELDS_INDENT)
    : "";
}

function updateOtherFieldValue(
  raw: string,
  key: string,
  value: unknown,
): string {
  const next = parseOtherFieldsForEdit(raw);
  if (
    value === undefined ||
    value === "" ||
    (Array.isArray(value) && value.length === 0)
  ) {
    delete next[key];
  } else {
    next[key] = value;
  }
  return stringifyOtherFieldsForEdit(next);
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function asBooleanOrUndefined(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function asObjectRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function asNumberOrDefault(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function asStringOrDefault(value: unknown, fallback: string): string {
  return typeof value === "string" ? value : fallback;
}

function parseNumberList(raw: string): number[] {
  return raw
    .split(",")
    .map((item) => Number(item.trim()))
    .filter((item) => Number.isFinite(item));
}

function formatNumberList(value: unknown, fallback: number[]): string {
  const numbers = Array.isArray(value)
    ? value.filter((item): item is number => typeof item === "number")
    : fallback;
  return numbers.join(",");
}

function toggleStringSelection(
  values: string[],
  value: string,
  checked: boolean,
): string[] {
  if (checked) {
    return values.includes(value) ? values : [...values, value];
  }
  return values.filter((item) => item !== value);
}

function parseJsonTemplate(template: string): unknown {
  return JSON.parse(template);
}

function getDefaultObjectValue(objectKey: string): Record<string, unknown> {
  switch (objectKey) {
    case "tmux":
      return parseJsonTemplate(OMO_TMUX_PLACEHOLDER) as Record<string, unknown>;
    case "runtime_fallback":
      return parseJsonTemplate(OMO_RUNTIME_FALLBACK_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "websearch":
      return parseJsonTemplate(OMO_WEBSEARCH_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "browser_automation_engine":
      return parseJsonTemplate(OMO_BROWSER_AUTOMATION_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "claude_code":
      return parseJsonTemplate(OMO_CLAUDE_CODE_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "sisyphus_agent":
      return parseJsonTemplate(OMO_SISYPHUS_AGENT_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "comment_checker":
      return parseJsonTemplate(OMO_COMMENT_CHECKER_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "ralph_loop":
      return parseJsonTemplate(OMO_RALPH_LOOP_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "model_capabilities":
      return parseJsonTemplate(OMO_MODEL_CAPABILITIES_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "babysitting":
      return parseJsonTemplate(OMO_BABYSITTING_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "git_master":
      return parseJsonTemplate(OMO_GIT_MASTER_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "start_work":
      return parseJsonTemplate(OMO_START_WORK_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "openclaw":
      return parseJsonTemplate(OMO_OPENCLAW_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "council":
      return parseJsonTemplate(OMO_SLIM_COUNCIL_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "multiplexer":
      return parseJsonTemplate(OMO_SLIM_MULTIPLEXER_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    case "todoContinuation":
      return parseJsonTemplate(
        OMO_SLIM_TODO_CONTINUATION_PLACEHOLDER,
      ) as Record<string, unknown>;
    case "fallback":
      return parseJsonTemplate(OMO_SLIM_FALLBACK_PLACEHOLDER) as Record<
        string,
        unknown
      >;
    default:
      return {};
  }
}

function collectCustomModels(
  store: Record<string, Record<string, unknown>>,
  builtinKeys: Set<string>,
): CustomModelItem[] {
  const customs: CustomModelItem[] = [];
  for (const [k, v] of Object.entries(store)) {
    if (!builtinKeys.has(k) && typeof v === "object" && v !== null) {
      customs.push({
        key: k,
        model: ((v as Record<string, unknown>).model as string) || "",
        sourceKey: k,
      });
    }
  }
  return customs;
}

export function mergeCustomModelsIntoStore(
  store: Record<string, Record<string, unknown>>,
  builtinKeys: Set<string>,
  customs: CustomModelItem[],
  modelVariantsMap: Record<string, string[]>,
  presetMetaMap: Record<string, PresetModelMeta> = {},
): Record<string, Record<string, unknown>> {
  const updated: Record<string, Record<string, unknown>> = {};

  for (const [key, value] of Object.entries(store)) {
    if (builtinKeys.has(key)) {
      updated[key] = { ...value };
    }
  }

  for (const custom of customs) {
    const targetKey = custom.key.trim();
    if (!targetKey) continue;

    const sourceKey = (custom.sourceKey || targetKey).trim();
    const sourceEntry = store[sourceKey] ?? store[targetKey];
    const nextEntry = {
      ...(updated[targetKey] || {}),
      ...(sourceEntry || {}),
    };

    if (custom.model.trim()) {
      nextEntry.model = custom.model;
      const currentVariant =
        typeof nextEntry.variant === "string" ? nextEntry.variant : "";
      if (currentVariant) {
        const validVariants = modelVariantsMap[custom.model] || [];
        if (!validVariants.includes(currentVariant)) {
          delete nextEntry.variant;
        }
      }
      updated[targetKey] = mergePresetMetaIntoEntry(
        nextEntry,
        custom.model,
        presetMetaMap,
      );
      continue;
    }

    delete nextEntry.model;
    delete nextEntry.variant;
    if (Object.keys(nextEntry).length > 0) {
      updated[targetKey] = nextEntry;
    } else {
      delete updated[targetKey];
    }
  }
  return updated;
}

export function OmoFormFields({
  modelOptions,
  modelVariantsMap = {},
  presetMetaMap = {},
  agents,
  onAgentsChange,
  categories = {},
  onCategoriesChange,
  otherFieldsStr,
  onOtherFieldsStrChange,
  isSlim = false,
}: OmoFormFieldsProps) {
  const { t } = useTranslation();

  const builtinAgentDefs = isSlim
    ? OMO_SLIM_BUILTIN_AGENTS
    : OMO_BUILTIN_AGENTS;
  const builtinAgentKeys = isSlim
    ? BUILTIN_AGENT_KEYS_SLIM
    : BUILTIN_AGENT_KEYS;

  const [mainAgentsOpen, setMainAgentsOpen] = useState(true);
  const [subAgentsOpen, setSubAgentsOpen] = useState(true);
  const [categoriesOpen, setCategoriesOpen] = useState(true);
  const [topLevelOpen, setTopLevelOpen] = useState(true);
  const [otherFieldsOpen, setOtherFieldsOpen] = useState(false);

  const [expandedAgents, setExpandedAgents] = useState<Record<string, boolean>>(
    {},
  );
  const [expandedCategories, setExpandedCategories] = useState<
    Record<string, boolean>
  >({});
  const [agentAdvancedDrafts, setAgentAdvancedDrafts] = useState<
    Record<string, string>
  >({});
  const [categoryAdvancedDrafts, setCategoryAdvancedDrafts] = useState<
    Record<string, string>
  >({});

  const [customAgents, setCustomAgents] = useState<CustomModelItem[]>(() =>
    collectCustomModels(agents, builtinAgentKeys),
  );

  const [customCategories, setCustomCategories] = useState<CustomModelItem[]>(
    () => collectCustomModels(categories, BUILTIN_CATEGORY_KEYS),
  );
  const parsedOtherFields = parseOtherFieldsForEdit(otherFieldsStr);

  useEffect(() => {
    setCustomAgents(collectCustomModels(agents, builtinAgentKeys));
  }, [agents]);

  useEffect(() => {
    setCustomCategories(collectCustomModels(categories, BUILTIN_CATEGORY_KEYS));
  }, [categories]);

  const syncCustomAgents = useCallback(
    (customs: CustomModelItem[]) => {
      onAgentsChange(
        mergeCustomModelsIntoStore(
          agents,
          builtinAgentKeys,
          customs,
          modelVariantsMap,
          presetMetaMap,
        ),
      );
    },
    [agents, onAgentsChange, modelVariantsMap, presetMetaMap, builtinAgentKeys],
  );

  const syncCustomCategories = useCallback(
    (customs: CustomModelItem[]) => {
      if (!onCategoriesChange) return;
      onCategoriesChange(
        mergeCustomModelsIntoStore(
          categories,
          BUILTIN_CATEGORY_KEYS,
          customs,
          modelVariantsMap,
          presetMetaMap,
        ),
      );
    },
    [categories, onCategoriesChange, modelVariantsMap, presetMetaMap],
  );

  const buildEffectiveModelOptions = useCallback(
    (currentModel: string): ModelOption[] => {
      if (!currentModel) return modelOptions;
      if (modelOptions.some((item) => item.value === currentModel)) {
        return modelOptions;
      }
      return [
        {
          value: currentModel,
          label: t("omo.currentValueNotEnabled", {
            value: currentModel,
            defaultValue: "{{value}} (current value, not enabled)",
          }),
        },
        ...modelOptions,
      ];
    },
    [modelOptions, t],
  );

  const resolveRecommendedModel = useCallback(
    (recommended?: string): string | undefined => {
      if (!recommended || modelOptions.length === 0) return undefined;

      const exact = modelOptions.find((item) => item.value === recommended);
      if (exact) return exact.value;

      const bySuffix = modelOptions.find((item) =>
        item.value.endsWith(`/${recommended}`),
      );
      return bySuffix?.value;
    },
    [modelOptions],
  );

  const renderModelSelect = (
    currentModel: string,
    onChange: (value: string) => void,
    recommended?: string,
  ) => {
    const options = buildEffectiveModelOptions(currentModel);
    return (
      <ModelCombobox
        value={currentModel}
        options={options}
        recommended={recommended}
        onChange={onChange}
      />
    );
  };

  const buildEffectiveVariantOptions = useCallback(
    (currentModel: string, currentVariant: string): string[] => {
      const variantKeys = modelVariantsMap[currentModel] || [];
      if (!currentVariant || variantKeys.includes(currentVariant)) {
        return variantKeys;
      }
      return [currentVariant, ...variantKeys];
    },
    [modelVariantsMap],
  );

  const updateOtherField = useCallback(
    (key: string, value: unknown) => {
      onOtherFieldsStrChange(updateOtherFieldValue(otherFieldsStr, key, value));
    },
    [onOtherFieldsStrChange, otherFieldsStr],
  );

  const updateDisabledList = useCallback(
    (key: string, option: string, checked: boolean) => {
      const current = asStringArray(parsedOtherFields[key]);
      updateOtherField(key, toggleStringSelection(current, option, checked));
    },
    [parsedOtherFields, updateOtherField],
  );

  const updateObjectField = useCallback(
    (objectKey: string, fieldKey: string, value: unknown) => {
      const current = {
        ...getDefaultObjectValue(objectKey),
        ...asObjectRecord(parsedOtherFields[objectKey]),
      };
      updateOtherField(objectKey, {
        ...current,
        [fieldKey]: value,
      });
    },
    [parsedOtherFields, updateOtherField],
  );

  const renderVariantSelect = (
    currentModel: string,
    currentVariant: string,
    onChange: (value: string) => void,
  ) => {
    const hasModel = Boolean(currentModel);
    const modelVariantKeys = hasModel
      ? modelVariantsMap[currentModel] || []
      : [];
    const hasVariants = modelVariantKeys.length > 0;
    const shouldShow = hasModel && (hasVariants || Boolean(currentVariant));

    if (!shouldShow) {
      return null;
    }

    const variantOptions = buildEffectiveVariantOptions(
      currentModel,
      currentVariant,
    );
    const firstIsUnavailable =
      Boolean(currentVariant) &&
      !(modelVariantsMap[currentModel] || []).includes(currentVariant);

    return (
      <Select
        value={currentVariant || EMPTY_VARIANT_VALUE}
        onValueChange={(value) =>
          onChange(value === EMPTY_VARIANT_VALUE ? "" : value)
        }
      >
        <SelectTrigger className="w-28 h-8 text-xs shrink-0">
          <SelectValue
            placeholder={t("omo.variantPlaceholder", {
              defaultValue: "variant",
            })}
          />
        </SelectTrigger>
        <SelectContent className="max-h-72">
          <SelectItem value={EMPTY_VARIANT_VALUE}>
            {t("omo.defaultWrapped", { defaultValue: "(Default)" })}
          </SelectItem>
          {variantOptions.map((variant, index) => (
            <SelectItem key={`${variant}-${index}`} value={variant}>
              {firstIsUnavailable && index === 0
                ? t("omo.currentValueUnavailable", {
                    value: variant,
                    defaultValue: "{{value}} (current value, unavailable)",
                  })
                : variant}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    );
  };

  const handleModelChange = (
    key: string,
    model: string,
    store: Record<string, Record<string, unknown>>,
    setter: (v: Record<string, Record<string, unknown>>) => void,
  ) => {
    if (model.trim()) {
      const nextEntry: Record<string, unknown> = {
        ...(store[key] || {}),
        model,
      };
      const currentVariant =
        typeof nextEntry.variant === "string" ? nextEntry.variant : "";
      if (currentVariant) {
        const validVariants = modelVariantsMap[model] || [];
        if (!validVariants.includes(currentVariant)) {
          delete nextEntry.variant;
        }
      }
      setter({
        ...store,
        [key]: mergePresetMetaIntoEntry(nextEntry, model, presetMetaMap),
      });
    } else {
      const existing = store[key];
      if (existing) {
        const adv = { ...existing };
        delete adv.model;
        delete adv.variant;
        if (Object.keys(adv).length > 0) {
          setter({ ...store, [key]: adv });
        } else {
          const next = { ...store };
          delete next[key];
          setter(next);
        }
      }
    }
  };

  const handleVariantChange = (
    key: string,
    variant: string,
    store: Record<string, Record<string, unknown>>,
    setter: (v: Record<string, Record<string, unknown>>) => void,
  ) => {
    const existing = store[key];
    if (variant.trim()) {
      setter({ ...store, [key]: { ...existing, variant } });
      return;
    }

    if (!existing) return;
    const nextEntry = { ...existing };
    delete nextEntry.variant;
    if (Object.keys(nextEntry).length > 0) {
      setter({ ...store, [key]: nextEntry });
      return;
    }

    const next = { ...store };
    delete next[key];
    setter(next);
  };

  const handleAdvancedChange = (
    key: string,
    rawJson: string,
    store: Record<string, Record<string, unknown>>,
    setter: (v: Record<string, Record<string, unknown>>) => void,
  ): boolean => {
    const currentModel = (store[key]?.model as string) || "";
    const currentVariant = (store[key]?.variant as string) || "";
    if (!rawJson.trim()) {
      if (currentModel || currentVariant) {
        setter({
          ...store,
          [key]: {
            ...(currentModel ? { model: currentModel } : {}),
            ...(currentVariant ? { variant: currentVariant } : {}),
          },
        });
      } else {
        const next = { ...store };
        delete next[key];
        setter(next);
      }
      return true;
    }
    try {
      const parsed = JSON.parse(rawJson);
      if (
        typeof parsed === "object" &&
        parsed !== null &&
        !Array.isArray(parsed)
      ) {
        const parsedAdvanced = { ...(parsed as Record<string, unknown>) };
        delete parsedAdvanced.model;
        delete parsedAdvanced.variant;
        setter({
          ...store,
          [key]: {
            ...(currentModel ? { model: currentModel } : {}),
            ...(currentVariant ? { variant: currentVariant } : {}),
            ...parsedAdvanced,
          },
        });
        return true;
      }
      return false;
    } catch {
      return false;
    }
  };

  type AdvancedScope = "agent" | "category";

  const setAdvancedDraft = (
    scope: AdvancedScope,
    key: string,
    value: string,
  ) => {
    if (scope === "agent") {
      setAgentAdvancedDrafts((prev) => ({ ...prev, [key]: value }));
      return;
    }
    setCategoryAdvancedDrafts((prev) => ({ ...prev, [key]: value }));
  };

  const removeAdvancedDraft = (scope: AdvancedScope, key: string) => {
    if (scope === "agent") {
      setAgentAdvancedDrafts((prev) => {
        const copied = { ...prev };
        delete copied[key];
        return copied;
      });
      return;
    }
    setCategoryAdvancedDrafts((prev) => {
      const copied = { ...prev };
      delete copied[key];
      return copied;
    });
  };

  const toggleAdvancedEditor = (
    scope: AdvancedScope,
    key: string,
    advStr: string,
    isExpanded: boolean,
  ) => {
    const willOpen = !isExpanded;
    if (scope === "agent") {
      setExpandedAgents((prev) => ({ ...prev, [key]: willOpen }));
      if (willOpen && agentAdvancedDrafts[key] === undefined) {
        setAdvancedDraft(scope, key, advStr);
      }
      return;
    }
    setExpandedCategories((prev) => ({ ...prev, [key]: willOpen }));
    if (willOpen && categoryAdvancedDrafts[key] === undefined) {
      setAdvancedDraft(scope, key, advStr);
    }
  };

  const renderAdvancedEditor = ({
    scope,
    draftKey,
    configKey,
    draftValue,
    store,
    setter,
    showHint,
  }: {
    scope: AdvancedScope;
    draftKey: string;
    configKey: string;
    draftValue: string;
    store: Record<string, Record<string, unknown>>;
    setter: (value: Record<string, Record<string, unknown>>) => void;
    showHint?: boolean;
  }) => (
    <div className="pb-2 pl-2 pr-2">
      <Textarea
        value={draftValue}
        onChange={(e) => setAdvancedDraft(scope, draftKey, e.target.value)}
        onBlur={(e) => {
          if (!handleAdvancedChange(configKey, e.target.value, store, setter)) {
            toast.error(
              t("omo.advancedJsonInvalid", {
                defaultValue: "Advanced JSON is invalid",
              }),
            );
          }
        }}
        placeholder={ADVANCED_PLACEHOLDER}
        className="font-mono text-xs min-h-[130px] py-3"
      />
      {showHint && (
        <p className="text-[10px] text-muted-foreground mt-1">
          {t("omo.advancedJsonHint", {
            defaultValue:
              "temperature, top_p, budgetTokens, prompt_append, permission, etc. Leave empty for defaults",
          })}
        </p>
      )}
    </div>
  );

  const handleFillAllRecommended = () => {
    if (modelOptions.length === 0) {
      toast.warning(
        t("omo.noEnabledModelsWarning", {
          defaultValue:
            "No configured models available. Configure OpenCode models first.",
        }),
      );
      return;
    }

    let filledCount = 0;
    let alreadySetCount = 0;
    let unmatchedCount = 0;

    const updatedAgents = { ...agents };
    for (const agentDef of builtinAgentDefs) {
      const recommendedValue = resolveRecommendedModel(agentDef.recommended);
      if (!recommendedValue) {
        unmatchedCount++;
      } else if (updatedAgents[agentDef.key]?.model) {
        alreadySetCount++;
      } else {
        updatedAgents[agentDef.key] = {
          ...mergePresetMetaIntoEntry(
            {
              ...updatedAgents[agentDef.key],
              model: recommendedValue,
            },
            recommendedValue,
            presetMetaMap,
          ),
        };
        filledCount++;
      }
    }
    onAgentsChange(updatedAgents);

    if (!isSlim && onCategoriesChange) {
      const updatedCategories = { ...categories };
      for (const catDef of OMO_BUILTIN_CATEGORIES) {
        const recommendedValue = resolveRecommendedModel(catDef.recommended);
        if (!recommendedValue) {
          unmatchedCount++;
        } else if (updatedCategories[catDef.key]?.model) {
          alreadySetCount++;
        } else {
          updatedCategories[catDef.key] = {
            ...mergePresetMetaIntoEntry(
              {
                ...updatedCategories[catDef.key],
                model: recommendedValue,
              },
              recommendedValue,
              presetMetaMap,
            ),
          };
          filledCount++;
        }
      }
      onCategoriesChange(updatedCategories);
    }

    if (filledCount > 0 && unmatchedCount === 0) {
      toast.success(
        t("omo.fillRecommendedSuccess", {
          defaultValue: "Filled {{count}} recommended models",
          count: filledCount,
        }),
      );
    } else if (filledCount > 0 && unmatchedCount > 0) {
      toast.success(
        t("omo.fillRecommendedPartial", {
          defaultValue:
            "Filled {{filled}} recommended models, {{unmatched}} unmatched",
          filled: filledCount,
          unmatched: unmatchedCount,
        }),
      );
    } else if (alreadySetCount > 0 && unmatchedCount === 0) {
      toast.info(
        t("omo.fillRecommendedAllSet", {
          defaultValue: "All slots already have models configured",
        }),
      );
    } else {
      toast.warning(
        t("omo.fillRecommendedNoMatch", {
          defaultValue: "Recommended models not found in configured providers",
        }),
      );
    }
  };

  const configuredAgentCount = Object.keys(agents).length;
  const configuredCategoryCount = isSlim ? 0 : Object.keys(categories).length;
  const mainAgents = builtinAgentDefs.filter((a) => a.group === "main");
  const subAgents = builtinAgentDefs.filter((a) => a.group === "sub");

  const readLocalFile = useReadOmoLocalFile();
  const readSlimLocalFile = useReadOmoSlimLocalFile();
  const isReadingLocalFile = isSlim
    ? readSlimLocalFile.isPending
    : readLocalFile.isPending;
  const [localFilePath, setLocalFilePath] = useState<string | null>(null);

  const handleImportFromLocal = useCallback(async () => {
    try {
      const data = isSlim
        ? await readSlimLocalFile.mutateAsync()
        : await readLocalFile.mutateAsync();
      const importedAgents =
        (data.agents as Record<string, Record<string, unknown>> | undefined) ||
        {};
      const importedCategories =
        (data.categories as
          | Record<string, Record<string, unknown>>
          | undefined) || {};

      onAgentsChange(importedAgents);
      if (!isSlim && onCategoriesChange) {
        onCategoriesChange(importedCategories);
      }
      onOtherFieldsStrChange(
        data.otherFields ? JSON.stringify(data.otherFields, null, 2) : "",
      );
      setAgentAdvancedDrafts({});
      setCategoryAdvancedDrafts({});
      setCustomAgents(collectCustomModels(importedAgents, builtinAgentKeys));
      if (!isSlim) {
        setCustomCategories(
          collectCustomModels(importedCategories, BUILTIN_CATEGORY_KEYS),
        );
      }
      setLocalFilePath(data.filePath);
      toast.success(
        t("omo.importLocalReplaceSuccess", {
          defaultValue:
            "Imported local file and replaced Agents/Categories/Other Fields",
        }),
      );
    } catch (err) {
      toast.error(
        t("omo.importLocalFailed", {
          error: String(err),
          defaultValue: "Failed to read local file: {{error}}",
        }),
      );
    }
  }, [
    readLocalFile,
    readSlimLocalFile,
    onAgentsChange,
    onCategoriesChange,
    onOtherFieldsStrChange,
    t,
  ]);

  const renderBuiltinModelRow = (
    scope: AdvancedScope,
    def: BuiltinModelDef,
  ) => {
    const isAgent = scope === "agent";
    const store = isAgent ? agents : categories;
    const setter = isAgent ? onAgentsChange : onCategoriesChange!;
    const drafts = isAgent ? agentAdvancedDrafts : categoryAdvancedDrafts;
    const expanded = isAgent ? expandedAgents : expandedCategories;

    const key = def.key;
    const currentModel = (store[key]?.model as string) || "";
    const currentVariant = (store[key]?.variant as string) || "";
    const advStr = getAdvancedStr(store[key]);
    const draftValue = drafts[key] ?? advStr;
    const isExpanded = expanded[key] ?? false;

    return (
      <div key={key} className="border-b border-border/30 last:border-b-0">
        <div className="flex items-center gap-2 py-1.5">
          <div className="w-32 shrink-0">
            <div className="flex items-center gap-1 text-sm font-medium">
              {def.display}
              <span className="relative inline-flex group/tip">
                <HelpCircle className="h-3.5 w-3.5 text-muted-foreground/60 hover:text-muted-foreground cursor-help shrink-0" />
                <span className="invisible opacity-0 group-hover/tip:visible group-hover/tip:opacity-100 transition-opacity duration-150 absolute left-0 top-full mt-1 z-50 w-[260px] rounded-md bg-popover text-popover-foreground border border-border shadow-md px-3 py-2 text-xs leading-relaxed font-normal pointer-events-none">
                  {t(def.tooltipKey)}
                </span>
              </span>
            </div>
            <div className="text-xs text-muted-foreground truncate">
              {t(def.descKey)}
            </div>
          </div>
          {renderModelSelect(
            currentModel,
            (value) => handleModelChange(key, value, store, setter),
            def.recommended,
          )}
          {renderVariantSelect(currentModel, currentVariant, (value) =>
            handleVariantChange(key, value, store, setter),
          )}
          <Button
            type="button"
            variant={isExpanded ? "secondary" : "ghost"}
            size="icon"
            className={cn("h-7 w-7 shrink-0", advStr && "text-primary")}
            onClick={() => toggleAdvancedEditor(scope, key, advStr, isExpanded)}
            title={t("omo.advancedLabel", { defaultValue: "Advanced" })}
          >
            <Settings className="h-3.5 w-3.5" />
          </Button>
        </div>
        {isExpanded &&
          renderAdvancedEditor({
            scope,
            draftKey: key,
            configKey: key,
            draftValue,
            store,
            setter,
            showHint: true,
          })}
      </div>
    );
  };

  const renderAgentRow = (agentDef: OmoAgentDef) =>
    renderBuiltinModelRow("agent", agentDef);

  const renderCategoryRow = (catDef: OmoCategoryDef) =>
    renderBuiltinModelRow("category", catDef);

  const renderCustomModelRow = (
    scope: AdvancedScope,
    item: CustomModelItem,
    index: number,
  ) => {
    const isAgent = scope === "agent";
    const store = isAgent ? agents : categories;
    const setter = isAgent ? onAgentsChange : onCategoriesChange!;
    const drafts = isAgent ? agentAdvancedDrafts : categoryAdvancedDrafts;
    const expanded = isAgent ? expandedAgents : expandedCategories;
    const customs = isAgent ? customAgents : customCategories;
    const setCustoms = isAgent ? setCustomAgents : setCustomCategories;
    const syncCustoms = isAgent ? syncCustomAgents : syncCustomCategories;

    const rowPrefix = isAgent ? "custom-agent" : "custom-cat";
    const emptyKeyPrefix = isAgent ? "__custom_agent_" : "__custom_cat_";
    const keyPlaceholder = isAgent
      ? t("omo.agentKeyPlaceholder", { defaultValue: "agent key" })
      : t("omo.categoryKeyPlaceholder", { defaultValue: "category key" });

    const key = item.key || `${emptyKeyPrefix}${index}`;
    const currentVariant =
      item.key && typeof store[item.key]?.variant === "string"
        ? (store[item.key]?.variant as string) || ""
        : "";
    const advStr = item.key ? getAdvancedStr(store[item.key]) : "";
    const draftValue = drafts[key] ?? advStr;
    const isExpanded = expanded[key] ?? false;

    const updateCustom = (patch: Partial<CustomModelItem>) => {
      const next = [...customs];
      next[index] = { ...next[index], ...patch };
      setCustoms(next);
      syncCustoms(next);
    };

    return (
      <div
        key={`${rowPrefix}-${index}`}
        className="border-b border-border/30 last:border-b-0"
      >
        <div className="flex items-center gap-2 py-1.5">
          <DeferredKeyInput
            value={item.key}
            onCommit={(value) => updateCustom({ key: value })}
            placeholder={keyPlaceholder}
            className="w-32 shrink-0 h-8 text-sm text-primary"
          />
          {renderModelSelect(item.model, (value) =>
            updateCustom({ model: value }),
          )}
          {renderVariantSelect(item.model, currentVariant, (value) => {
            if (!item.key) return;
            handleVariantChange(item.key, value, store, setter);
          })}
          <Button
            type="button"
            variant={isExpanded ? "secondary" : "ghost"}
            size="icon"
            className={cn("h-7 w-7 shrink-0", advStr && "text-primary")}
            onClick={() => toggleAdvancedEditor(scope, key, advStr, isExpanded)}
            title={t("omo.advancedLabel", { defaultValue: "Advanced" })}
          >
            <Settings className="h-3.5 w-3.5" />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="h-7 w-7 shrink-0 text-destructive"
            onClick={() => {
              const next = customs.filter((_, idx) => idx !== index);
              setCustoms(next);
              syncCustoms(next);
              removeAdvancedDraft(scope, key);
            }}
          >
            <Trash2 className="h-3.5 w-3.5" />
          </Button>
        </div>
        {isExpanded &&
          item.key &&
          renderAdvancedEditor({
            scope,
            draftKey: key,
            configKey: item.key,
            draftValue,
            store,
            setter,
          })}
      </div>
    );
  };

  const SectionHeader = ({
    title,
    isOpen,
    onToggle,
    badge,
    action,
  }: {
    title: string;
    isOpen: boolean;
    onToggle: () => void;
    badge?: React.ReactNode | string;
    action?: React.ReactNode;
  }) => (
    <div className="flex items-center justify-between w-full py-2 px-3">
      <button
        type="button"
        className="flex min-w-0 flex-1 items-center gap-2 text-left"
        onClick={onToggle}
      >
        {isOpen ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-4 w-4 text-muted-foreground" />
        )}
        <Label className="text-sm font-semibold cursor-pointer">{title}</Label>
        {typeof badge === "string" ? (
          <Badge variant="outline" className="text-[10px] h-5">
            {badge}
          </Badge>
        ) : (
          badge
        )}
      </button>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );

  const renderModelSection = ({
    title,
    isOpen,
    onToggle,
    badge,
    action,
    maxHeightClass = "max-h-[5000px]",
    children,
  }: {
    title: string;
    isOpen: boolean;
    onToggle: () => void;
    badge?: React.ReactNode | string;
    action?: React.ReactNode;
    maxHeightClass?: string;
    children: React.ReactNode;
  }) => (
    <div className="rounded-lg border border-border/60">
      <SectionHeader
        title={title}
        isOpen={isOpen}
        onToggle={onToggle}
        badge={badge}
        action={action}
      />
      <div
        className={cn(
          "overflow-hidden transition-all duration-200",
          isOpen ? `${maxHeightClass} opacity-100` : "max-h-0 opacity-0",
        )}
      >
        <div className="px-3 pb-3">{children}</div>
      </div>
    </div>
  );

  const renderCustomAddButton = (onClick: () => void) => (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      className="h-6 text-xs"
      onClick={onClick}
    >
      <Plus className="h-3.5 w-3.5 mr-1" />
      {t("omo.custom", { defaultValue: "Custom" })}
    </Button>
  );

  const renderCustomDivider = (label: string) => (
    <div className="flex items-center gap-2 py-2">
      <div className="flex-1 border-t border-border/40" />
      <span className="text-[10px] text-muted-foreground">{label}</span>
      <div className="flex-1 border-t border-border/40" />
    </div>
  );

  const renderBooleanField = (key: string, label: string) => {
    const checked = asBooleanOrUndefined(parsedOtherFields[key]) ?? false;
    const fieldId = `omo-top-level-${key}`;
    return (
      <div className="flex items-center justify-between gap-3 rounded-md border border-border/40 px-3 py-2">
        <Label htmlFor={fieldId} className="text-sm">
          {label}
        </Label>
        <Switch
          id={fieldId}
          checked={checked}
          onCheckedChange={(value) => updateOtherField(key, value)}
        />
      </div>
    );
  };

  const renderDisabledChecklist = (
    key: string,
    label: string,
    options: readonly SelectableOption[],
  ) => {
    const values = asStringArray(parsedOtherFields[key]);
    return (
      <div className="space-y-2">
        <Label className="text-xs font-medium text-muted-foreground">
          {label}
        </Label>
        <div className="grid grid-cols-1 gap-1.5 sm:grid-cols-2">
          {options.map((option) => {
            const optionId = `omo-${key}-${option.value.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
            return (
              <div
                key={option.value}
                className="flex min-w-0 items-center gap-2 rounded-md border border-border/40 px-2 py-1.5"
              >
                <Checkbox
                  id={optionId}
                  checked={values.includes(option.value)}
                  onCheckedChange={(checked) =>
                    updateDisabledList(key, option.value, checked === true)
                  }
                />
                <Label
                  htmlFor={optionId}
                  className="min-w-0 flex-1 truncate text-xs font-normal"
                >
                  {option.label}
                </Label>
              </div>
            );
          })}
        </div>
      </div>
    );
  };

  const renderObjectTemplateButtons = (
    title: string,
    templates: Array<{ key: string; label: string; template: string }>,
  ) => (
    <div className="space-y-2">
      <Label className="text-xs font-medium text-muted-foreground">
        {title}
      </Label>
      <div className="flex flex-wrap gap-1.5">
        {templates.map((item) => (
          <Button
            key={item.key}
            type="button"
            variant={parsedOtherFields[item.key] ? "secondary" : "outline"}
            size="sm"
            className="h-7 text-xs"
            onClick={() =>
              updateOtherField(item.key, parseJsonTemplate(item.template))
            }
          >
            {item.label}
          </Button>
        ))}
      </div>
    </div>
  );

  const renderObjectSwitch = (
    objectKey: string,
    fieldKey: string,
    label: string,
    defaultValue = false,
  ) => {
    const objectValue = asObjectRecord(parsedOtherFields[objectKey]);
    const checked = asBooleanOrUndefined(objectValue[fieldKey]) ?? defaultValue;
    const fieldId = `omo-${objectKey}-${fieldKey}`;
    return (
      <div className="flex items-center justify-between gap-3 rounded-md border border-border/40 px-3 py-2">
        <Label htmlFor={fieldId} className="text-xs font-normal">
          {label}
        </Label>
        <Switch
          id={fieldId}
          checked={checked}
          onCheckedChange={(value) =>
            updateObjectField(objectKey, fieldKey, value)
          }
        />
      </div>
    );
  };

  const renderObjectNumberInput = (
    objectKey: string,
    fieldKey: string,
    label: string,
    defaultValue: number,
    min?: number,
    max?: number,
  ) => {
    const objectValue = asObjectRecord(parsedOtherFields[objectKey]);
    const fieldId = `omo-${objectKey}-${fieldKey}`;
    return (
      <div className="space-y-1.5">
        <Label htmlFor={fieldId} className="text-xs">
          {label}
        </Label>
        <Input
          id={fieldId}
          type="number"
          min={min}
          max={max}
          value={asNumberOrDefault(objectValue[fieldKey], defaultValue)}
          onChange={(event) =>
            updateObjectField(objectKey, fieldKey, Number(event.target.value))
          }
        />
      </div>
    );
  };

  const renderObjectSelect = (
    objectKey: string,
    fieldKey: string,
    label: string,
    values: string[],
    defaultValue: string,
  ) => {
    const objectValue = asObjectRecord(parsedOtherFields[objectKey]);
    const current = asStringOrDefault(objectValue[fieldKey], defaultValue);
    return (
      <div className="space-y-1.5">
        <Label className="text-xs">{label}</Label>
        <Select
          value={current}
          onValueChange={(value) =>
            updateObjectField(objectKey, fieldKey, value)
          }
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {values.map((value) => (
              <SelectItem key={value} value={value}>
                {value}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    );
  };

  const renderTmuxFields = () => (
    <div className="space-y-3 rounded-md border border-border/50 p-3">
      <Label className="text-xs font-semibold">tmux</Label>
      <div className="grid gap-3 sm:grid-cols-2">
        {renderObjectSwitch("tmux", "enabled", "enabled")}
        {renderObjectSelect(
          "tmux",
          "layout",
          "layout",
          [
            "main-horizontal",
            "main-vertical",
            "tiled",
            "even-horizontal",
            "even-vertical",
          ],
          "main-vertical",
        )}
        {renderObjectNumberInput(
          "tmux",
          "main_pane_size",
          "main_pane_size",
          60,
          20,
          80,
        )}
        {renderObjectNumberInput(
          "tmux",
          "main_pane_min_width",
          "main_pane_min_width",
          120,
          40,
        )}
        {renderObjectNumberInput(
          "tmux",
          "agent_pane_min_width",
          "agent_pane_min_width",
          40,
          20,
        )}
        {renderObjectSelect(
          "tmux",
          "isolation",
          "isolation",
          ["inline", "window", "session"],
          "inline",
        )}
      </div>
    </div>
  );

  const renderRuntimeFallbackFields = () => {
    const runtimeFallback = asObjectRecord(parsedOtherFields.runtime_fallback);
    return (
      <div className="space-y-3 rounded-md border border-border/50 p-3">
        <Label className="text-xs font-semibold">runtime_fallback</Label>
        <div className="grid gap-3 sm:grid-cols-2">
          {renderObjectSwitch("runtime_fallback", "enabled", "enabled", true)}
          {renderObjectSwitch(
            "runtime_fallback",
            "notify_on_fallback",
            "notify_on_fallback",
            true,
          )}
          {renderObjectNumberInput(
            "runtime_fallback",
            "max_fallback_attempts",
            "max_fallback_attempts",
            3,
            1,
            20,
          )}
          {renderObjectNumberInput(
            "runtime_fallback",
            "cooldown_seconds",
            "cooldown_seconds",
            30,
            0,
          )}
          {renderObjectNumberInput(
            "runtime_fallback",
            "timeout_seconds",
            "timeout_seconds",
            0,
            0,
          )}
          <div className="space-y-1.5">
            <Label
              htmlFor="omo-runtime-fallback-retry-errors"
              className="text-xs"
            >
              retry_on_errors
            </Label>
            <Input
              id="omo-runtime-fallback-retry-errors"
              value={formatNumberList(
                runtimeFallback.retry_on_errors,
                [429, 500, 502, 503, 504],
              )}
              onChange={(event) =>
                updateObjectField(
                  "runtime_fallback",
                  "retry_on_errors",
                  parseNumberList(event.target.value),
                )
              }
              placeholder="429,500,502,503,504"
            />
          </div>
        </div>
      </div>
    );
  };

  const renderProviderObjectFields = () => (
    <div className="grid gap-3 sm:grid-cols-2">
      {renderObjectSelect(
        "websearch",
        "provider",
        "websearch.provider",
        ["exa", "tavily"],
        "exa",
      )}
      {!isSlim &&
        renderObjectSelect(
          "browser_automation_engine",
          "provider",
          "browser_automation_engine.provider",
          ["playwright", "agent-browser", "dev-browser", "playwright-cli"],
          "playwright",
        )}
    </div>
  );

  const renderClaudeCodeFields = () =>
    !isSlim && (
      <div className="space-y-3 rounded-md border border-border/50 p-3">
        <Label className="text-xs font-semibold">claude_code</Label>
        <div className="grid gap-2 sm:grid-cols-3">
          {["mcp", "commands", "skills", "agents", "hooks", "plugins"].map(
            (key) => (
              <div key={key}>
                {renderObjectSwitch("claude_code", key, key, true)}
              </div>
            ),
          )}
        </div>
      </div>
    );

  const renderStandardMoreObjectFields = () =>
    !isSlim && (
      <div className="space-y-3 rounded-md border border-border/50 p-3">
        <Label className="text-xs font-semibold">
          {t("omo.standardObjects", { defaultValue: "Standard Objects" })}
        </Label>
        <div className="grid gap-3 sm:grid-cols-2">
          {renderObjectSwitch(
            "sisyphus_agent",
            "disabled",
            "sisyphus_agent.disabled",
          )}
          {renderObjectSwitch(
            "sisyphus_agent",
            "default_builder_enabled",
            "sisyphus_agent.default_builder_enabled",
          )}
          {renderObjectSwitch(
            "sisyphus_agent",
            "planner_enabled",
            "sisyphus_agent.planner_enabled",
            true,
          )}
          {renderObjectSwitch(
            "sisyphus_agent",
            "replace_plan",
            "sisyphus_agent.replace_plan",
            true,
          )}
          {renderObjectSwitch(
            "sisyphus_agent",
            "tdd",
            "sisyphus_agent.tdd",
            true,
          )}
          {renderObjectSwitch(
            "start_work",
            "auto_commit",
            "start_work.auto_commit",
            true,
          )}
          {renderObjectNumberInput(
            "babysitting",
            "timeout_ms",
            "babysitting.timeout_ms",
            120000,
            0,
          )}
          {renderObjectSwitch(
            "git_master",
            "commit_footer",
            "git_master.commit_footer",
            true,
          )}
          {renderObjectSwitch(
            "git_master",
            "include_co_authored_by",
            "git_master.include_co_authored_by",
            true,
          )}
          <div className="space-y-1.5">
            <Label htmlFor="omo-git-master-env-prefix" className="text-xs">
              git_master.git_env_prefix
            </Label>
            <Input
              id="omo-git-master-env-prefix"
              value={asStringOrDefault(
                asObjectRecord(parsedOtherFields.git_master).git_env_prefix,
                "GIT_MASTER=1",
              )}
              onChange={(event) =>
                updateObjectField(
                  "git_master",
                  "git_env_prefix",
                  event.target.value,
                )
              }
            />
          </div>
          {renderObjectSwitch("ralph_loop", "enabled", "ralph_loop.enabled")}
          {renderObjectNumberInput(
            "ralph_loop",
            "default_max_iterations",
            "ralph_loop.default_max_iterations",
            100,
            1,
            1000,
          )}
          {renderObjectSelect(
            "ralph_loop",
            "default_strategy",
            "ralph_loop.default_strategy",
            ["reset", "continue"],
            "continue",
          )}
          {renderObjectSwitch(
            "model_capabilities",
            "enabled",
            "model_capabilities.enabled",
            true,
          )}
          {renderObjectSwitch(
            "model_capabilities",
            "auto_refresh_on_start",
            "model_capabilities.auto_refresh_on_start",
            true,
          )}
          {renderObjectNumberInput(
            "model_capabilities",
            "refresh_timeout_ms",
            "model_capabilities.refresh_timeout_ms",
            10000,
            1,
          )}
          <div className="space-y-1.5">
            <Label
              htmlFor="omo-model-capabilities-source-url"
              className="text-xs"
            >
              model_capabilities.source_url
            </Label>
            <Input
              id="omo-model-capabilities-source-url"
              value={asStringOrDefault(
                asObjectRecord(parsedOtherFields.model_capabilities).source_url,
                "",
              )}
              onChange={(event) =>
                updateObjectField(
                  "model_capabilities",
                  "source_url",
                  event.target.value,
                )
              }
            />
          </div>
          <div className="space-y-1.5 sm:col-span-2">
            <Label
              htmlFor="omo-comment-checker-custom-prompt"
              className="text-xs"
            >
              comment_checker.custom_prompt
            </Label>
            <Input
              id="omo-comment-checker-custom-prompt"
              value={asStringOrDefault(
                asObjectRecord(parsedOtherFields.comment_checker).custom_prompt,
                "",
              )}
              onChange={(event) =>
                updateObjectField(
                  "comment_checker",
                  "custom_prompt",
                  event.target.value,
                )
              }
            />
          </div>
          {renderObjectSwitch("openclaw", "enabled", "openclaw.enabled")}
        </div>
      </div>
    );

  const renderSlimObjectFields = () =>
    isSlim && (
      <div className="space-y-3 rounded-md border border-border/50 p-3">
        <Label className="text-xs font-semibold">
          {t("omo.slimObjects", { defaultValue: "Slim Objects" })}
        </Label>
        <div className="grid gap-3 sm:grid-cols-2">
          {renderObjectSwitch("council", "enabled", "council.enabled", true)}
          {renderObjectNumberInput(
            "council",
            "minVotes",
            "council.minVotes",
            2,
            1,
          )}
          {renderObjectSwitch(
            "multiplexer",
            "enabled",
            "multiplexer.enabled",
            true,
          )}
          {renderObjectNumberInput(
            "multiplexer",
            "maxConcurrent",
            "multiplexer.maxConcurrent",
            3,
            1,
          )}
          {renderObjectSwitch(
            "todoContinuation",
            "enabled",
            "todoContinuation.enabled",
            true,
          )}
          {renderObjectSwitch("fallback", "enabled", "fallback.enabled", true)}
          {renderObjectNumberInput(
            "fallback",
            "maxAttempts",
            "fallback.maxAttempts",
            3,
            1,
          )}
        </div>
      </div>
    );

  const renderTopLevelOptions = () => {
    const schemaValue =
      typeof parsedOtherFields.$schema === "string"
        ? parsedOtherFields.$schema
        : "";
    const defaultRunAgent =
      typeof parsedOtherFields.default_run_agent === "string"
        ? parsedOtherFields.default_run_agent
        : "";
    const disabledAgents = isSlim
      ? OMO_SLIM_DISABLEABLE_AGENTS
      : OMO_DISABLEABLE_AGENTS;
    const disabledMcps = isSlim
      ? OMO_SLIM_DISABLEABLE_MCPS
      : OMO_DISABLEABLE_MCPS;
    const disabledHooks = isSlim
      ? OMO_SLIM_DISABLEABLE_HOOKS
      : OMO_DISABLEABLE_HOOKS;
    const schemaUrl = isSlim
      ? OMO_SLIM_DEFAULT_SCHEMA_URL
      : OMO_DEFAULT_SCHEMA_URL;
    const commonTemplates = [
      {
        key: "runtime_fallback",
        label: "runtime_fallback",
        template: OMO_RUNTIME_FALLBACK_PLACEHOLDER,
      },
      { key: "tmux", label: "tmux", template: OMO_TMUX_PLACEHOLDER },
      {
        key: "websearch",
        label: "websearch",
        template: OMO_WEBSEARCH_PLACEHOLDER,
      },
      {
        key: "background_task",
        label: "background_task",
        template: OMO_BACKGROUND_TASK_PLACEHOLDER,
      },
    ];
    const standardTemplates = [
      {
        key: "claude_code",
        label: "claude_code",
        template: OMO_CLAUDE_CODE_PLACEHOLDER,
      },
      {
        key: "experimental",
        label: "experimental",
        template: OMO_EXPERIMENTAL_PLACEHOLDER,
      },
      {
        key: "browser_automation_engine",
        label: "browser_automation_engine",
        template: OMO_BROWSER_AUTOMATION_PLACEHOLDER,
      },
      {
        key: "sisyphus_agent",
        label: "sisyphus_agent",
        template: OMO_SISYPHUS_AGENT_PLACEHOLDER,
      },
      {
        key: "comment_checker",
        label: "comment_checker",
        template: OMO_COMMENT_CHECKER_PLACEHOLDER,
      },
      {
        key: "ralph_loop",
        label: "ralph_loop",
        template: OMO_RALPH_LOOP_PLACEHOLDER,
      },
      {
        key: "model_capabilities",
        label: "model_capabilities",
        template: OMO_MODEL_CAPABILITIES_PLACEHOLDER,
      },
      {
        key: "babysitting",
        label: "babysitting",
        template: OMO_BABYSITTING_PLACEHOLDER,
      },
      {
        key: "git_master",
        label: "git_master",
        template: OMO_GIT_MASTER_PLACEHOLDER,
      },
      {
        key: "start_work",
        label: "start_work",
        template: OMO_START_WORK_PLACEHOLDER,
      },
      {
        key: "openclaw",
        label: "openclaw",
        template: OMO_OPENCLAW_PLACEHOLDER,
      },
    ];
    const slimTemplates = [
      {
        key: "council",
        label: "council",
        template: OMO_SLIM_COUNCIL_PLACEHOLDER,
      },
      {
        key: "multiplexer",
        label: "multiplexer",
        template: OMO_SLIM_MULTIPLEXER_PLACEHOLDER,
      },
      {
        key: "todoContinuation",
        label: "todoContinuation",
        template: OMO_SLIM_TODO_CONTINUATION_PLACEHOLDER,
      },
      {
        key: "fallback",
        label: "fallback",
        template: OMO_SLIM_FALLBACK_PLACEHOLDER,
      },
    ];

    return (
      <div className="space-y-4">
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label htmlFor="omo-schema-url" className="text-xs">
              {t("omo.schemaUrl", { defaultValue: "Schema URL" })}
            </Label>
            <div className="flex gap-2">
              <Input
                id="omo-schema-url"
                value={schemaValue}
                onChange={(event) =>
                  updateOtherField("$schema", event.target.value.trim())
                }
                placeholder={schemaUrl}
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="shrink-0"
                onClick={() => updateOtherField("$schema", schemaUrl)}
              >
                {t("omo.useDefault", { defaultValue: "Default" })}
              </Button>
            </div>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="omo-default-run-agent" className="text-xs">
              {t("omo.defaultRunAgent", { defaultValue: "Default Run Agent" })}
            </Label>
            <Input
              id="omo-default-run-agent"
              value={defaultRunAgent}
              onChange={(event) =>
                updateOtherField("default_run_agent", event.target.value.trim())
              }
              placeholder={isSlim ? "orchestrator" : "sisyphus"}
            />
          </div>
        </div>

        <div className="grid gap-2 sm:grid-cols-2">
          {renderBooleanField(
            "new_task_system_enabled",
            t("omo.newTaskSystemEnabled", {
              defaultValue: "New Task System",
            }),
          )}
          {renderBooleanField(
            "auto_update",
            t("omo.autoUpdate", { defaultValue: "Auto Update" }),
          )}
          {renderBooleanField(
            "model_fallback",
            t("omo.modelFallback", { defaultValue: "Model Fallback" }),
          )}
          {renderBooleanField(
            "hashline_edit",
            t("omo.hashlineEdit", { defaultValue: "Hashline Edit" }),
          )}
        </div>

        {renderDisabledChecklist(
          "disabled_agents",
          t("omo.disabledAgents", { defaultValue: "Disabled Agents" }),
          disabledAgents,
        )}
        {renderDisabledChecklist(
          "disabled_mcps",
          t("omo.disabledMcps", { defaultValue: "Disabled MCPs" }),
          disabledMcps,
        )}
        {renderDisabledChecklist(
          "disabled_hooks",
          t("omo.disabledHooks", { defaultValue: "Disabled Hooks" }),
          disabledHooks,
        )}
        {!isSlim &&
          renderDisabledChecklist(
            "disabled_skills",
            t("omo.disabledSkills", { defaultValue: "Disabled Skills" }),
            OMO_DISABLEABLE_SKILLS,
          )}
        {!isSlim &&
          renderDisabledChecklist(
            "disabled_commands",
            t("omo.disabledCommands", { defaultValue: "Disabled Commands" }),
            OMO_DISABLEABLE_COMMANDS,
          )}
        {!isSlim &&
          renderDisabledChecklist(
            "disabled_tools",
            t("omo.disabledTools", { defaultValue: "Disabled Tools" }),
            OMO_DISABLEABLE_TOOLS,
          )}
        <div className="space-y-1.5">
          <Label htmlFor="omo-mcp-env-allowlist" className="text-xs">
            {t("omo.mcpEnvAllowlist", {
              defaultValue: "MCP Env Allowlist",
            })}
          </Label>
          <Input
            id="omo-mcp-env-allowlist"
            value={asStringArray(parsedOtherFields.mcp_env_allowlist).join(",")}
            onChange={(event) =>
              updateOtherField(
                "mcp_env_allowlist",
                event.target.value
                  .split(",")
                  .map((item) => item.trim())
                  .filter(Boolean),
              )
            }
            placeholder="PATH,HOME,SHELL"
          />
        </div>
        {renderRuntimeFallbackFields()}
        {renderTmuxFields()}
        {renderProviderObjectFields()}
        {renderClaudeCodeFields()}
        {renderStandardMoreObjectFields()}
        {renderSlimObjectFields()}
        {renderObjectTemplateButtons(
          t("omo.objectTemplates", { defaultValue: "Object Templates" }),
          [...commonTemplates, ...(isSlim ? slimTemplates : standardTemplates)],
        )}
      </div>
    );
  };

  const addCustomModel = (scope: AdvancedScope) => {
    if (scope === "agent") {
      setCustomAgents((prev) => [
        ...prev,
        { key: "", model: "", sourceKey: "" },
      ]);
      setSubAgentsOpen(true);
      return;
    }
    setCustomCategories((prev) => [
      ...prev,
      { key: "", model: "", sourceKey: "" },
    ]);
    setCategoriesOpen(true);
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <Label className="text-sm font-semibold">
          {t("omo.modelConfiguration", { defaultValue: "Model Configuration" })}
        </Label>
        <div className="flex items-center gap-1.5">
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            disabled={isReadingLocalFile}
            onClick={handleImportFromLocal}
          >
            {isReadingLocalFile ? (
              <Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />
            ) : (
              <FolderInput className="h-3.5 w-3.5 mr-1" />
            )}
            {t("omo.importLocal", { defaultValue: "Import Local" })}
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 text-xs"
            onClick={handleFillAllRecommended}
          >
            <Wand2 className="h-3.5 w-3.5 mr-1" />
            {t("omo.fillRecommended", { defaultValue: "Fill Recommended" })}
          </Button>
        </div>
      </div>

      <div className="text-xs text-muted-foreground">
        {t("omo.configSummary", {
          agents: configuredAgentCount,
          categories: configuredCategoryCount,
          defaultValue:
            "{{agents}} agents, {{categories}} categories configured · Click ⚙ for advanced params",
        })}
        <span className="ml-1">
          ·{" "}
          {t("omo.enabledModelsCount", {
            count: modelOptions.length,
            defaultValue: "{{count}} configured models available",
          })}
        </span>
        {localFilePath && (
          <span className="ml-1 text-primary/70">
            · {t("omo.source", { defaultValue: "from:" })}{" "}
            <span className="font-mono text-[10px]">
              {localFilePath.replace(/^.*\//, "")}
            </span>
          </span>
        )}
      </div>

      {renderModelSection({
        title: t("omo.mainAgents", { defaultValue: "Main Agents" }),
        isOpen: mainAgentsOpen,
        onToggle: () => setMainAgentsOpen(!mainAgentsOpen),
        badge: `${mainAgents.length}`,
        children: mainAgents.map(renderAgentRow),
      })}

      {renderModelSection({
        title: t("omo.subAgents", { defaultValue: "Sub Agents" }),
        isOpen: subAgentsOpen,
        onToggle: () => setSubAgentsOpen(!subAgentsOpen),
        badge: `${subAgents.length + customAgents.length}`,
        action: renderCustomAddButton(() => addCustomModel("agent")),
        children: (
          <>
            {subAgents.map(renderAgentRow)}
            {customAgents.length > 0 && (
              <>
                {renderCustomDivider(
                  t("omo.customAgents", { defaultValue: "Custom Agents" }),
                )}
                {customAgents.map((a, i) =>
                  renderCustomModelRow("agent", a, i),
                )}
              </>
            )}
          </>
        ),
      })}

      {!isSlim &&
        renderModelSection({
          title: t("omo.categories", { defaultValue: "Categories" }),
          isOpen: categoriesOpen,
          onToggle: () => setCategoriesOpen(!categoriesOpen),
          badge: `${OMO_BUILTIN_CATEGORIES.length + customCategories.length}`,
          action: renderCustomAddButton(() => addCustomModel("category")),
          children: (
            <>
              {OMO_BUILTIN_CATEGORIES.map(renderCategoryRow)}
              {customCategories.length > 0 && (
                <>
                  {renderCustomDivider(
                    t("omo.customCategories", {
                      defaultValue: "Custom Categories",
                    }),
                  )}
                  {customCategories.map((c, i) =>
                    renderCustomModelRow("category", c, i),
                  )}
                </>
              )}
            </>
          ),
        })}

      {renderModelSection({
        title: t("omo.topLevelOptions", {
          defaultValue: "Top-Level Options",
        }),
        isOpen: topLevelOpen,
        onToggle: () => setTopLevelOpen(!topLevelOpen),
        badge: `${Object.keys(parsedOtherFields).length}`,
        maxHeightClass: "max-h-[3000px]",
        children: renderTopLevelOptions(),
      })}

      {renderModelSection({
        title: t("omo.otherFieldsJson", {
          defaultValue: "Other Fields (JSON)",
        }),
        isOpen: otherFieldsOpen,
        onToggle: () => setOtherFieldsOpen(!otherFieldsOpen),
        badge:
          !otherFieldsOpen && otherFieldsStr.trim() ? (
            <Badge
              variant="secondary"
              className="text-[10px] h-5 font-mono max-w-[200px] truncate"
            >
              {otherFieldsStr.trim().slice(0, 40)}
              {otherFieldsStr.trim().length > 40 ? "..." : ""}
            </Badge>
          ) : undefined,
        maxHeightClass: "max-h-[500px]",
        children: (
          <>
            <Textarea
              value={otherFieldsStr}
              onChange={(e) => onOtherFieldsStrChange(e.target.value)}
              placeholder='{ "custom_key": "value" }'
              className="font-mono text-xs min-h-[60px]"
            />
            {isSlim && (
              <p className="mt-1 text-[10px] text-muted-foreground">
                {t("omo.slimOtherFieldsHint", {
                  defaultValue:
                    "Use this area for top-level OMO Slim config such as council, fallback, multiplexer, disabled_mcps, and todoContinuation.",
                })}
              </p>
            )}
          </>
        ),
      })}
    </div>
  );
}
