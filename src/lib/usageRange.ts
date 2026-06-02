import type { UsageRangePreset, UsageRangeSelection } from "@/types/usage";

export interface ResolvedUsageRange {
  startDate: number;
  endDate: number;
}

const DAY_MS = 24 * 60 * 60 * 1000;

export function startOfToday(now = new Date()): number {
  return new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
}

export function endOfDay(timeMs: number): number {
  const day = new Date(timeMs);
  return new Date(
    day.getFullYear(),
    day.getMonth(),
    day.getDate(),
    23,
    59,
    59,
    999,
  ).getTime();
}

export function usageRangeAroundLatestData(
  latestDataAt: number,
  days = 7,
): UsageRangeSelection {
  const safeDays = Math.max(1, Math.floor(days));
  const latestDayStart = startOfToday(new Date(latestDataAt));
  return {
    preset: "custom",
    customStartDate: latestDayStart - (safeDays - 1) * DAY_MS,
    customEndDate: endOfDay(latestDataAt),
  };
}

export function resolveUsageRange(
  selection: UsageRangeSelection,
): ResolvedUsageRange {
  const now = Date.now();
  if (selection.preset === "custom") {
    return {
      startDate: selection.customStartDate ?? now - 7 * DAY_MS,
      endDate: selection.customEndDate ?? now,
    };
  }
  if (selection.preset === "today") {
    return { startDate: startOfToday(), endDate: now };
  }
  if (selection.preset === "all") {
    return { startDate: 0, endDate: now };
  }
  const days = Number.parseInt(selection.preset, 10);
  return {
    startDate: now - (Number.isFinite(days) ? days : 7) * DAY_MS,
    endDate: now,
  };
}

export function usageRangeLabel(preset: UsageRangePreset): string {
  switch (preset) {
    case "today":
      return "Today";
    case "1d":
      return "24h";
    case "7d":
      return "7d";
    case "14d":
      return "14d";
    case "30d":
      return "30d";
    case "all":
      return "All time";
    case "custom":
      return "Custom";
  }
}
