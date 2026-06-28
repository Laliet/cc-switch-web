import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { BarChart3, Coins, FileText, RefreshCw, Server } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  useModelStats,
  useProviderStats,
  useUsageDataExtent,
  useUsageSummary,
} from "@/lib/query/usage";
import {
  AppTypeFilter,
  KNOWN_USAGE_APP_TYPES,
  UsageRangeSelection,
  UsageStatsFilters,
  usageAppLabel,
} from "@/types/usage";
import {
  startOfToday,
  usageRangeAroundLatestData,
  usageRangeLabel,
  resolveUsageRange,
} from "@/lib/usageRange";
import { getLocaleFromLanguage } from "./format";
import { UsageHero } from "./UsageHero";
import { UsageTrendChart } from "./UsageTrendChart";
import { RequestLogTable } from "./RequestLogTable";
import { ProviderStatsTable } from "./ProviderStatsTable";
import { ModelStatsTable } from "./ModelStatsTable";
import { PricingConfigPanel } from "./PricingConfigPanel";
import { DataSourceBar } from "./DataSourceBar";
import { UsageDateRangePicker } from "./UsageDateRangePicker";

const APP_FILTERS: AppTypeFilter[] = ["all", ...KNOWN_USAGE_APP_TYPES];

export function UsageDashboard() {
  const { t, i18n } = useTranslation();
  const [range, setRange] = useState<UsageRangeSelection>({ preset: "today" });
  const [appType, setAppType] = useState<AppTypeFilter>("all");
  const [providerId, setProviderId] = useState("all");
  const [model, setModel] = useState("all");
  const [refreshIntervalMs, setRefreshIntervalMs] = useState(30_000);
  const [rangeWasSelected, setRangeWasSelected] = useState(false);
  const [autoRangeApplied, setAutoRangeApplied] = useState(false);
  const statsFilters = useMemo<UsageStatsFilters>(
    () => ({
      providerId: providerId === "all" ? undefined : providerId,
      model: model === "all" ? undefined : model,
    }),
    [model, providerId],
  );
  const todaySummary = useUsageSummary(
    { preset: "today" },
    appType,
    statsFilters,
    refreshIntervalMs,
  );
  const dataExtent = useUsageDataExtent(appType, refreshIntervalMs);
  const providerOptionsQuery = useProviderStats(
    range,
    appType,
    model === "all" ? undefined : { model },
    refreshIntervalMs,
  );
  const modelOptionsQuery = useModelStats(
    range,
    appType,
    providerId === "all" ? undefined : { providerId },
    refreshIntervalMs,
  );
  const usageLoadError =
    todaySummary.error instanceof Error
      ? todaySummary.error.message
      : dataExtent.error instanceof Error
        ? dataExtent.error.message
        : null;

  const refreshLabel = useMemo(
    () => (refreshIntervalMs > 0 ? `${refreshIntervalMs / 1000}s` : "Off"),
    [refreshIntervalMs],
  );
  const resolvedRange = useMemo(() => resolveUsageRange(range), [range]);
  const rangeLabel = useMemo(() => {
    if (range.preset !== "custom") {
      return usageRangeLabel(range.preset);
    }
    const locale = getLocaleFromLanguage(
      i18n.resolvedLanguage || i18n.language,
    );
    return `${new Date(resolvedRange.startDate).toLocaleString(locale)} - ${new Date(
      resolvedRange.endDate,
    ).toLocaleString(locale)}`;
  }, [
    i18n.language,
    i18n.resolvedLanguage,
    range.preset,
    resolvedRange.endDate,
    resolvedRange.startDate,
  ]);

  const providerOptions = useMemo(() => {
    const options = new Map<
      string,
      { value: string; label: string; appTypes: Set<string> }
    >();
    for (const provider of providerOptionsQuery.data ?? []) {
      const existing = options.get(provider.providerId);
      if (existing) {
        existing.appTypes.add(provider.appType);
        continue;
      }
      options.set(provider.providerId, {
        value: provider.providerId,
        label: provider.providerName || provider.providerId,
        appTypes: new Set([provider.appType]),
      });
    }
    return Array.from(options.values()).map((option) => ({
      ...option,
      appTypes: Array.from(option.appTypes),
    }));
  }, [providerOptionsQuery.data]);

  const modelOptions = useMemo(
    () => (modelOptionsQuery.data ?? []).map((item) => item.model),
    [modelOptionsQuery.data],
  );

  const cycleRefresh = () => {
    const values = [0, 5000, 10000, 30000, 60000];
    const index = values.indexOf(refreshIntervalMs);
    setRefreshIntervalMs(values[(index + 1) % values.length] ?? 30000);
  };

  useEffect(() => {
    const extent = dataExtent.data;
    const summary = todaySummary.data;
    if (
      rangeWasSelected ||
      autoRangeApplied ||
      range.preset !== "today" ||
      !extent?.lastSeenAt ||
      extent.requestCount <= 0 ||
      !summary ||
      summary.totalRequests > 0 ||
      extent.lastSeenAt >= startOfToday()
    ) {
      return;
    }

    setRange(usageRangeAroundLatestData(extent.lastSeenAt, 7));
    setAutoRangeApplied(true);
  }, [
    autoRangeApplied,
    dataExtent.data,
    range.preset,
    rangeWasSelected,
    todaySummary.data,
  ]);

  useEffect(() => {
    if (
      providerId !== "all" &&
      providerOptions.length > 0 &&
      !providerOptions.some((option) => option.value === providerId)
    ) {
      setProviderId("all");
    }
  }, [providerId, providerOptions]);

  useEffect(() => {
    if (
      model !== "all" &&
      modelOptions.length > 0 &&
      !modelOptions.includes(model)
    ) {
      setModel("all");
    }
  }, [model, modelOptions]);

  return (
    <div className="space-y-5 pb-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-2xl font-semibold tracking-normal">
            {t("usage.title", { defaultValue: "Usage Dashboard" })}
          </h2>
          <p className="text-sm text-muted-foreground">
            {t("usage.subtitle", {
              defaultValue:
                "Proxy request logs, token usage, model pricing, and cost allocation.",
            })}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <UsageDateRangePicker
            selection={range}
            triggerLabel={rangeLabel}
            onApply={(value) => {
              setRangeWasSelected(true);
              setRange(value);
            }}
          />
          <Select
            value={appType}
            onValueChange={(value) => {
              setAppType(value as AppTypeFilter);
              setProviderId("all");
              setModel("all");
            }}
          >
            <SelectTrigger className="w-[140px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {APP_FILTERS.map((app) => (
                <SelectItem key={app} value={app}>
                  {usageAppLabel(app)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={providerId} onValueChange={setProviderId}>
            <SelectTrigger className="w-[180px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">
                {t("usage.allProviders", { defaultValue: "All providers" })}
              </SelectItem>
              {providerOptions.map((provider) => (
                <SelectItem key={provider.value} value={provider.value}>
                  {provider.label}
                  {appType === "all" && provider.appTypes.length === 1
                    ? ` (${usageAppLabel(provider.appTypes[0])})`
                    : ""}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={model} onValueChange={setModel}>
            <SelectTrigger className="w-[190px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">
                {t("usage.allModels", { defaultValue: "All models" })}
              </SelectItem>
              {modelOptions.map((modelOption) => (
                <SelectItem key={modelOption} value={modelOption}>
                  {modelOption}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button variant="outline" onClick={cycleRefresh}>
            <RefreshCw className="h-4 w-4" />
            {refreshLabel}
          </Button>
        </div>
      </div>

      <DataSourceBar refreshIntervalMs={refreshIntervalMs} />
      {usageLoadError ? (
        <Alert variant="destructive">
          <AlertDescription>
            {t("usage.loadFailed", {
              defaultValue: "Usage data failed to load: {{error}}",
              error: usageLoadError,
            })}
          </AlertDescription>
        </Alert>
      ) : null}
      <UsageHero
        range={range}
        appType={appType}
        filters={statsFilters}
        refreshIntervalMs={refreshIntervalMs}
      />
      <UsageTrendChart
        range={range}
        appType={appType}
        filters={statsFilters}
        refreshIntervalMs={refreshIntervalMs}
      />

      <Tabs defaultValue="logs" className="w-full">
        <TabsList className="mb-3 flex h-auto flex-wrap justify-start">
          <TabsTrigger value="logs" className="gap-2">
            <FileText className="h-4 w-4" />
            Logs
          </TabsTrigger>
          <TabsTrigger value="providers" className="gap-2">
            <Server className="h-4 w-4" />
            Providers
          </TabsTrigger>
          <TabsTrigger value="models" className="gap-2">
            <BarChart3 className="h-4 w-4" />
            Models
          </TabsTrigger>
          <TabsTrigger value="pricing" className="gap-2">
            <Coins className="h-4 w-4" />
            Pricing
          </TabsTrigger>
        </TabsList>
        <TabsContent value="logs">
          <RequestLogTable
            range={range}
            rangeLabel={rangeLabel}
            appType={appType}
            filters={statsFilters}
            refreshIntervalMs={refreshIntervalMs}
            onRangeChange={(nextRange) => {
              setRangeWasSelected(true);
              setRange(nextRange);
            }}
          />
        </TabsContent>
        <TabsContent value="providers">
          <ProviderStatsTable
            range={range}
            appType={appType}
            filters={statsFilters}
            refreshIntervalMs={refreshIntervalMs}
          />
        </TabsContent>
        <TabsContent value="models">
          <ModelStatsTable
            range={range}
            appType={appType}
            filters={statsFilters}
            refreshIntervalMs={refreshIntervalMs}
          />
        </TabsContent>
        <TabsContent value="pricing">
          <PricingConfigPanel />
        </TabsContent>
      </Tabs>
    </div>
  );
}

export default UsageDashboard;
