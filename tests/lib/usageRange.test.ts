import { afterEach, describe, expect, it, vi } from "vitest";
import {
  endOfDay,
  resolveUsageRange,
  usageRangeAroundLatestData,
} from "@/lib/usageRange";

describe("usage range helpers", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("builds a calendar window around the latest usage data", () => {
    const latest = new Date("2025-12-27T10:30:00+08:00").getTime();
    const range = usageRangeAroundLatestData(latest, 7);

    expect(range).toEqual({
      preset: "custom",
      customStartDate: new Date(2025, 11, 21).getTime(),
      customEndDate: new Date(2025, 11, 27, 23, 59, 59, 999).getTime(),
    });
  });

  it("finds the local end of day for a timestamp", () => {
    const time = new Date(2025, 9, 4, 8, 15, 0, 0).getTime();

    expect(endOfDay(time)).toBe(
      new Date(2025, 9, 4, 23, 59, 59, 999).getTime(),
    );
  });

  it("resolves all-time ranges from epoch to now", () => {
    const now = new Date("2026-06-02T08:00:00+08:00").getTime();
    vi.spyOn(Date, "now").mockReturnValue(now);

    expect(resolveUsageRange({ preset: "all" })).toEqual({
      startDate: 0,
      endDate: now,
    });
  });
});
