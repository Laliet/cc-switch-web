import { useEffect, useMemo, useState } from "react";
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
  UsageRangePreset,
  UsageRangeSelection,
  UsageStatsFilters,
  usageAppLabel,
} from "@/types/usage";
import {
  startOfToday,
  usageRangeAroundLatestData,
  usageRangeLabel,
} from "@/lib/usageRange";
import { UsageHero } from "./UsageHero";
import { UsageTrendChart } from "./UsageTrendChart";
import { RequestLogTable } from "./RequestLogTable";
import { ProviderStatsTable } from "./ProviderStatsTable";
import { ModelStatsTable } from "./ModelStatsTable";
import { PricingConfigPanel } from "./PricingConfigPanel";
import { DataSourceBar } from "./DataSourceBar";

const RANGE_PRESETS: UsageRangePreset[] = [
  "today",
  "1d",
  "7d",
  "14d",
  "30d",
  "all",
];
const APP_FILTERS: AppTypeFilter[] = ["all", ...KNOWN_USAGE_APP_TYPES];

export function UsageDashboard() {
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

  const providerOptions = useMemo(
    () => {
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
    },
    [providerOptionsQuery.data],
  );

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
            Usage Dashboard
          </h2>
          <p className="text-sm text-muted-foreground">
            Proxy request logs, token usage, model pricing, and cost allocation.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Select
            value={range.preset}
            onValueChange={(value) => {
              setRangeWasSelected(true);
              setRange({ preset: value as UsageRangePreset });
            }}
          >
            <SelectTrigger className="w-[120px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {RANGE_PRESETS.map((preset) => (
                <SelectItem key={preset} value={preset}>
                  {usageRangeLabel(preset)}
                </SelectItem>
              ))}
              {range.preset === "custom" ? (
                <SelectItem value="custom">Recent data</SelectItem>
              ) : null}
            </SelectContent>
          </Select>
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
              <SelectItem value="all">All providers</SelectItem>
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
              <SelectItem value="all">All models</SelectItem>
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
            Usage data failed to load: {usageLoadError}
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
            appType={appType}
            filters={statsFilters}
            refreshIntervalMs={refreshIntervalMs}
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
