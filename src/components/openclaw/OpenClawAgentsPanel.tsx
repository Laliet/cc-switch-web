import { useEffect, useMemo, useState } from "react";
import { Plus, Save, Trash2, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  useOpenClawAgents,
  useOpenClawStatus,
  useSaveOpenClawAgents,
} from "@/hooks/useOpenClaw";
import type {
  OpenClawAgentsDefaults,
  OpenClawModelCatalogEntry,
} from "@/lib/api";
import { extractErrorMessage } from "@/utils/errorUtils";

const UNSET = "__openclaw_unset__";

interface OpenClawAgentsPanelProps {
  enabled: boolean;
}

export function OpenClawAgentsPanel({ enabled }: OpenClawAgentsPanelProps) {
  const { t } = useTranslation();
  const agentsQuery = useOpenClawAgents(enabled);
  const statusQuery = useOpenClawStatus(enabled);
  const saveMutation = useSaveOpenClawAgents();
  const [source, setSource] = useState<OpenClawAgentsDefaults>({});
  const [primary, setPrimary] = useState("");
  const [fallbacks, setFallbacks] = useState<string[]>([]);
  const [workspace, setWorkspace] = useState("");
  const [timeoutSeconds, setTimeoutSeconds] = useState("");
  const [contextTokens, setContextTokens] = useState("");
  const [maxConcurrent, setMaxConcurrent] = useState("");
  const [catalog, setCatalog] = useState<
    Record<string, OpenClawModelCatalogEntry>
  >({});

  useEffect(() => {
    if (!agentsQuery.data) return;
    const value = agentsQuery.data.value ?? {};
    setSource(value);
    setPrimary(value.model?.primary ?? "");
    setFallbacks(value.model?.fallbacks ?? []);
    setWorkspace(value.workspace ?? "");
    const timeout = value.timeoutSeconds ?? value.timeout;
    setTimeoutSeconds(timeout === undefined ? "" : String(timeout));
    setContextTokens(
      value.contextTokens === undefined ? "" : String(value.contextTokens),
    );
    setMaxConcurrent(
      value.maxConcurrent === undefined ? "" : String(value.maxConcurrent),
    );
    setCatalog(value.models ?? {});
  }, [agentsQuery.data]);

  const modelOptions = useMemo(() => {
    const values =
      statusQuery.data?.providers.flatMap((provider) =>
        provider.models.map((model) => ({
          value: `${provider.id}/${model.id}`,
          label: model.name
            ? `${provider.id} / ${model.name}`
            : `${provider.id} / ${model.id}`,
        })),
      ) ?? [];
    const known = new Set(values.map((item) => item.value));
    for (const current of [primary, ...fallbacks, ...Object.keys(catalog)]) {
      if (current && !known.has(current)) {
        values.push({ value: current, label: current });
        known.add(current);
      }
    }
    return values.sort((left, right) => left.value.localeCompare(right.value));
  }, [catalog, fallbacks, primary, statusQuery.data]);

  const parsePositive = (value: string, field: string) => {
    if (!value.trim()) return undefined;
    const parsed = Number(value);
    if (!Number.isFinite(parsed) || parsed <= 0) {
      throw new Error(`${field}: ${t("openclaw.config.positiveRequired")}`);
    }
    return parsed;
  };

  const save = async () => {
    if (!agentsQuery.data) return;
    try {
      const next: OpenClawAgentsDefaults = { ...source };
      if (primary) {
        next.model = {
          ...(source.model ?? {}),
          primary,
          fallbacks: fallbacks.filter(Boolean),
        };
      } else {
        delete next.model;
      }
      next.models = Object.keys(catalog).length > 0 ? catalog : undefined;
      next.workspace = workspace.trim() || undefined;
      next.timeoutSeconds = parsePositive(
        timeoutSeconds,
        t("openclaw.config.timeout"),
      );
      next.contextTokens = parsePositive(
        contextTokens,
        t("openclaw.config.contextTokens"),
      );
      next.maxConcurrent = parsePositive(
        maxConcurrent,
        t("openclaw.config.maxConcurrent"),
      );
      delete next.timeout;
      await saveMutation.mutateAsync({
        defaults: next,
        expectedEtag: agentsQuery.data.etag,
      });
      toast.success(t("openclaw.config.saved"));
    } catch (error) {
      toast.error(t("openclaw.config.saveFailed"), {
        description: extractErrorMessage(error),
      });
    }
  };

  if (agentsQuery.isLoading || statusQuery.isLoading) {
    return <PanelLoading />;
  }

  const hasLegacyTimeout =
    typeof agentsQuery.data?.value?.timeout === "number" &&
    typeof agentsQuery.data?.value?.timeoutSeconds !== "number";

  return (
    <div className="min-h-0 overflow-y-auto">
      <section className="border-b px-5 py-4">
        {hasLegacyTimeout ? (
          <Alert className="mb-4 border-amber-500/30 bg-amber-500/5">
            <div className="flex items-start gap-3">
              <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0 text-amber-600" />
              <div>
                <p className="font-medium">
                  {t("openclaw.config.legacyTimeoutTitle")}
                </p>
                <AlertDescription className="mt-1 text-muted-foreground">
                  {t("openclaw.config.legacyTimeoutDescription")}
                </AlertDescription>
              </div>
            </div>
          </Alert>
        ) : null}
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="sm:col-span-2">
            <Label>{t("openclaw.config.primaryModel")}</Label>
            <Select
              value={primary || UNSET}
              onValueChange={(value) =>
                setPrimary(value === UNSET ? "" : value)
              }
            >
              <SelectTrigger className="mt-1.5 font-mono text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={UNSET}>{t("common.none")}</SelectItem>
                {modelOptions.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div>
            <Label htmlFor="openclaw-workspace">
              {t("openclaw.config.workspace")}
            </Label>
            <Input
              id="openclaw-workspace"
              className="mt-1.5 font-mono text-xs"
              value={workspace}
              onChange={(event) => setWorkspace(event.target.value)}
            />
          </div>
          <div>
            <Label htmlFor="openclaw-timeout">
              {t("openclaw.config.timeout")}
            </Label>
            <Input
              id="openclaw-timeout"
              type="number"
              min="1"
              className="mt-1.5"
              value={timeoutSeconds}
              onChange={(event) => setTimeoutSeconds(event.target.value)}
            />
          </div>
          <div>
            <Label htmlFor="openclaw-context">
              {t("openclaw.config.contextTokens")}
            </Label>
            <Input
              id="openclaw-context"
              type="number"
              min="1"
              className="mt-1.5"
              value={contextTokens}
              onChange={(event) => setContextTokens(event.target.value)}
            />
          </div>
          <div>
            <Label htmlFor="openclaw-concurrency">
              {t("openclaw.config.maxConcurrent")}
            </Label>
            <Input
              id="openclaw-concurrency"
              type="number"
              min="1"
              className="mt-1.5"
              value={maxConcurrent}
              onChange={(event) => setMaxConcurrent(event.target.value)}
            />
          </div>
        </div>
      </section>

      <section className="border-b px-5 py-4">
        <div className="mb-3 flex items-center justify-between gap-2">
          <h3 className="text-sm font-medium">
            {t("openclaw.config.fallbackModels")}
          </h3>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => setFallbacks((current) => [...current, ""])}
          >
            <Plus className="h-4 w-4" />
            {t("common.add")}
          </Button>
        </div>
        <div className="space-y-2">
          {fallbacks.map((fallback, index) => (
            <div
              className="flex items-center gap-2"
              key={`${index}-${fallback}`}
            >
              <Select
                value={fallback || UNSET}
                onValueChange={(value) =>
                  setFallbacks((current) =>
                    current.map((item, itemIndex) =>
                      itemIndex === index
                        ? value === UNSET
                          ? ""
                          : value
                        : item,
                    ),
                  )
                }
              >
                <SelectTrigger className="min-w-0 flex-1 font-mono text-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={UNSET}>{t("common.none")}</SelectItem>
                  {modelOptions.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Button
                type="button"
                size="icon"
                variant="ghost"
                title={t("common.delete")}
                onClick={() =>
                  setFallbacks((current) =>
                    current.filter((_, itemIndex) => itemIndex !== index),
                  )
                }
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          ))}
        </div>
      </section>

      <section className="px-5 py-4">
        <h3 className="mb-3 text-sm font-medium">
          {t("openclaw.config.modelCatalog")}
        </h3>
        <div className="divide-y rounded-md border">
          {modelOptions.map((option) => {
            const entry = catalog[option.value];
            return (
              <div
                key={option.value}
                className="grid min-h-14 grid-cols-[auto_minmax(0,1fr)] items-center gap-3 px-3 py-2 sm:grid-cols-[auto_minmax(180px,1fr)_220px]"
              >
                <Checkbox
                  checked={Boolean(entry)}
                  aria-label={option.value}
                  onCheckedChange={(checked) =>
                    setCatalog((current) => {
                      const next = { ...current };
                      if (checked)
                        next[option.value] = current[option.value] ?? {};
                      else delete next[option.value];
                      return next;
                    })
                  }
                />
                <span
                  className="min-w-0 truncate font-mono text-xs"
                  title={option.value}
                >
                  {option.value}
                </span>
                <Input
                  value={entry?.alias ?? ""}
                  disabled={!entry}
                  placeholder={t("openclaw.config.alias")}
                  className="col-start-2 text-sm sm:col-start-auto"
                  onChange={(event) =>
                    setCatalog((current) => ({
                      ...current,
                      [option.value]: {
                        ...(current[option.value] ?? {}),
                        alias: event.target.value || undefined,
                      },
                    }))
                  }
                />
              </div>
            );
          })}
          {modelOptions.length === 0 ? (
            <div className="px-3 py-8 text-center text-sm text-muted-foreground">
              {t("openclaw.config.noModels")}
            </div>
          ) : null}
        </div>
      </section>

      <div className="sticky bottom-0 flex justify-end border-t bg-background px-5 py-3">
        <Button onClick={() => void save()} disabled={saveMutation.isPending}>
          <Save className="h-4 w-4" />
          {saveMutation.isPending ? t("common.saving") : t("common.save")}
        </Button>
      </div>
    </div>
  );
}

function PanelLoading() {
  const { t } = useTranslation();
  return (
    <div className="grid min-h-64 place-items-center text-sm text-muted-foreground">
      {t("common.loading")}
    </div>
  );
}
