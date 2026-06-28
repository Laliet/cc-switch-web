import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import { resolveUsageRange, usageRangeLabel } from "@/lib/usageRange";
import type { UsageRangePreset, UsageRangeSelection } from "@/types/usage";
import { getLocaleFromLanguage } from "./format";

type DraftField = "start" | "end";

const PRESETS: UsageRangePreset[] = ["today", "1d", "7d", "14d", "30d"];

interface UsageDateRangePickerProps {
  selection: UsageRangeSelection;
  triggerLabel: string;
  onApply: (selection: UsageRangeSelection) => void;
}

function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function endOfDay(date: Date): Date {
  return new Date(
    date.getFullYear(),
    date.getMonth(),
    date.getDate(),
    23,
    59,
    59,
    999,
  );
}

function isSameDay(left: Date, right: Date): boolean {
  return (
    left.getFullYear() === right.getFullYear() &&
    left.getMonth() === right.getMonth() &&
    left.getDate() === right.getDate()
  );
}

function formatDateInput(timeMs: number): string {
  const date = new Date(timeMs);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(
    2,
    "0",
  )}-${String(date.getDate()).padStart(2, "0")}`;
}

function formatTimeInput(timeMs: number): string {
  const date = new Date(timeMs);
  return `${String(date.getHours()).padStart(2, "0")}:${String(
    date.getMinutes(),
  ).padStart(2, "0")}`;
}

function parseDateInput(timeMs: number, value: string): number {
  const [year, month, day] = value.split("-").map(Number);
  if (
    !Number.isFinite(year) ||
    !Number.isFinite(month) ||
    !Number.isFinite(day)
  ) {
    return timeMs;
  }
  const base = new Date(timeMs);
  return new Date(
    year,
    month - 1,
    day,
    base.getHours(),
    base.getMinutes(),
    base.getSeconds(),
    base.getMilliseconds(),
  ).getTime();
}

function parseTimeInput(timeMs: number, value: string): number {
  const [hour, minute] = value.split(":").map(Number);
  if (!Number.isFinite(hour) || !Number.isFinite(minute)) return timeMs;
  const base = new Date(timeMs);
  return new Date(
    base.getFullYear(),
    base.getMonth(),
    base.getDate(),
    hour,
    minute,
    base.getSeconds(),
    base.getMilliseconds(),
  ).getTime();
}

function setDateKeepTime(timeMs: number, day: Date): number {
  const base = new Date(timeMs);
  return new Date(
    day.getFullYear(),
    day.getMonth(),
    day.getDate(),
    base.getHours(),
    base.getMinutes(),
    base.getSeconds(),
    base.getMilliseconds(),
  ).getTime();
}

function getCalendarDays(month: Date): Date[] {
  const first = new Date(month.getFullYear(), month.getMonth(), 1);
  const gridStart = new Date(first);
  gridStart.setDate(first.getDate() - first.getDay());
  return Array.from({ length: 42 }, (_, index) => {
    const date = new Date(gridStart);
    date.setDate(gridStart.getDate() + index);
    return date;
  });
}

export function UsageDateRangePicker({
  selection,
  triggerLabel,
  onApply,
}: UsageDateRangePickerProps) {
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const [activeField, setActiveField] = useState<DraftField>("start");
  const resolvedRange = useMemo(
    () => resolveUsageRange(selection),
    [selection],
  );
  const [draftStart, setDraftStart] = useState(resolvedRange.startDate);
  const [draftEnd, setDraftEnd] = useState(resolvedRange.endDate);
  const [displayMonth, setDisplayMonth] = useState(
    () =>
      new Date(
        new Date(resolvedRange.startDate).getFullYear(),
        new Date(resolvedRange.startDate).getMonth(),
        1,
      ),
  );
  const [error, setError] = useState<string | null>(null);

  const locale = getLocaleFromLanguage(i18n.resolvedLanguage || i18n.language);
  const calendarDays = useMemo(
    () => getCalendarDays(displayMonth),
    [displayMonth],
  );
  const weekdayLabels = useMemo(
    () =>
      Array.from({ length: 7 }, (_, index) =>
        new Intl.DateTimeFormat(locale, { weekday: "narrow" }).format(
          new Date(2024, 0, 7 + index),
        ),
      ),
    [locale],
  );

  useEffect(() => {
    if (!open) return;
    const nextRange = resolveUsageRange(selection);
    setDraftStart(nextRange.startDate);
    setDraftEnd(nextRange.endDate);
    setDisplayMonth(
      new Date(
        new Date(nextRange.startDate).getFullYear(),
        new Date(nextRange.startDate).getMonth(),
        1,
      ),
    );
    setActiveField("start");
    setError(null);
  }, [open, selection]);

  const handleDatePick = (day: Date) => {
    setError(null);
    const nextTs = setDateKeepTime(
      activeField === "start" ? draftStart : draftEnd,
      day,
    );
    if (activeField === "start") {
      setDraftStart(nextTs);
      if (nextTs > draftEnd) setDraftEnd(endOfDay(day).getTime());
      setActiveField("end");
    } else if (nextTs < draftStart) {
      setDraftStart(startOfDay(day).getTime());
      setActiveField("end");
    } else {
      setDraftEnd(nextTs);
    }
    if (
      day.getMonth() !== displayMonth.getMonth() ||
      day.getFullYear() !== displayMonth.getFullYear()
    ) {
      setDisplayMonth(new Date(day.getFullYear(), day.getMonth(), 1));
    }
  };

  const handleApply = () => {
    setError(null);
    if (draftStart > draftEnd) {
      setError(
        t("usage.invalidTimeRangeOrder", {
          defaultValue: "Start time cannot be later than end time",
        }),
      );
      return;
    }
    onApply({
      preset: "custom",
      customStartDate: draftStart,
      customEndDate: draftEnd,
    });
    setOpen(false);
  };

  const renderField = (field: DraftField) => {
    const active = activeField === field;
    const value = field === "start" ? draftStart : draftEnd;
    const update = field === "start" ? setDraftStart : setDraftEnd;
    const label =
      field === "start"
        ? t("usage.startTime", { defaultValue: "Start time" })
        : t("usage.endTime", { defaultValue: "End time" });

    return (
      <div
        className={cn(
          "rounded-md border px-3 py-2 transition-colors",
          active
            ? "border-primary bg-primary/5 ring-1 ring-primary/30"
            : "border-border-default hover:border-border",
        )}
        onClick={() => setActiveField(field)}
      >
        <div className="mb-1.5 text-[11px] font-medium uppercase text-muted-foreground">
          {label}
        </div>
        <div className="flex items-center gap-1.5">
          <Input
            type="date"
            className="h-7 flex-1 border-0 bg-transparent p-0 text-sm shadow-none focus-visible:ring-0"
            value={formatDateInput(value)}
            onChange={(event) => {
              const next = parseDateInput(value, event.target.value);
              update(next);
              const date = new Date(next);
              setDisplayMonth(new Date(date.getFullYear(), date.getMonth(), 1));
              setError(null);
            }}
            onFocus={() => setActiveField(field)}
          />
          <Input
            type="time"
            step={60}
            className="h-7 w-[88px] border-0 bg-transparent p-0 text-sm shadow-none focus-visible:ring-0"
            value={formatTimeInput(value)}
            onChange={(event) => {
              update(parseTimeInput(value, event.target.value));
              setError(null);
            }}
            onFocus={() => setActiveField(field)}
          />
        </div>
      </div>
    );
  };

  const startDay = new Date(draftStart);
  const endDay = new Date(draftEnd);
  const today = new Date();

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant={selection.preset === "custom" ? "default" : "outline"}
          className="min-w-[150px] justify-start gap-2"
        >
          <CalendarDays className="h-4 w-4" />
          <span className="truncate">{triggerLabel}</span>
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        className="w-[340px] max-w-[calc(100vw-2rem)] p-3 sm:w-[620px]"
      >
        <div className="flex flex-wrap gap-1.5 border-b border-border-default pb-2">
          {PRESETS.map((preset) => (
            <Button
              key={preset}
              type="button"
              size="sm"
              variant={selection.preset === preset ? "default" : "outline"}
              className="h-7 px-2.5 text-xs"
              onClick={() => {
                onApply({ preset });
                setOpen(false);
              }}
            >
              {usageRangeLabel(preset)}
            </Button>
          ))}
        </div>

        <div className="flex flex-col gap-3 pt-3 sm:flex-row">
          <div className="space-y-2 sm:w-[250px] sm:flex-none">
            <p className="text-xs text-muted-foreground">
              {t("usage.customRangeHint", {
                defaultValue: "Pick exact dates and times.",
              })}
            </p>
            {renderField("start")}
            {renderField("end")}
            {error ? <p className="text-xs text-destructive">{error}</p> : null}
            <div className="flex gap-2 pt-1">
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="flex-1"
                onClick={() => setOpen(false)}
              >
                {t("common.cancel", { defaultValue: "Cancel" })}
              </Button>
              <Button
                type="button"
                size="sm"
                className="flex-1"
                onClick={handleApply}
              >
                {t("common.confirm", { defaultValue: "Confirm" })}
              </Button>
            </div>
          </div>

          <div className="rounded-md border border-border-default bg-muted/30 p-2.5 sm:flex-1">
            <div className="mb-1.5 flex items-center justify-between">
              <Button
                type="button"
                size="icon"
                variant="ghost"
                className="h-7 w-7"
                onClick={() =>
                  setDisplayMonth(
                    new Date(
                      displayMonth.getFullYear(),
                      displayMonth.getMonth() - 1,
                      1,
                    ),
                  )
                }
              >
                <ChevronLeft className="h-3.5 w-3.5" />
              </Button>
              <button
                type="button"
                className="text-sm font-medium hover:text-primary"
                onClick={() =>
                  setDisplayMonth(
                    new Date(today.getFullYear(), today.getMonth(), 1),
                  )
                }
              >
                {displayMonth.toLocaleDateString(locale, {
                  year: "numeric",
                  month: "long",
                })}
              </button>
              <Button
                type="button"
                size="icon"
                variant="ghost"
                className="h-7 w-7"
                onClick={() =>
                  setDisplayMonth(
                    new Date(
                      displayMonth.getFullYear(),
                      displayMonth.getMonth() + 1,
                      1,
                    ),
                  )
                }
              >
                <ChevronRight className="h-3.5 w-3.5" />
              </Button>
            </div>
            <div className="mb-0.5 grid grid-cols-7 text-center text-[11px] text-muted-foreground">
              {weekdayLabels.map((label, index) => (
                <div key={`${label}-${index}`} className="py-0.5">
                  {label}
                </div>
              ))}
            </div>
            <div className="grid grid-cols-7 gap-px">
              {calendarDays.map((day) => {
                const currentMonth = day.getMonth() === displayMonth.getMonth();
                const dayStart = startOfDay(day);
                const inRange =
                  dayStart >= startOfDay(startDay) &&
                  dayStart <= startOfDay(endDay);
                const endpoint =
                  isSameDay(day, startDay) || isSameDay(day, endDay);
                return (
                  <button
                    key={day.toISOString()}
                    type="button"
                    aria-label={day.toLocaleDateString(locale)}
                    className={cn(
                      "relative h-7 rounded text-xs transition-colors",
                      !currentMonth && "text-muted-foreground/30",
                      currentMonth && !inRange && "hover:bg-muted",
                      inRange && !endpoint && "bg-primary/10 text-primary",
                      endpoint &&
                        "bg-primary font-medium text-primary-foreground",
                      isSameDay(day, today) &&
                        !endpoint &&
                        "ring-1 ring-primary/40",
                    )}
                    onClick={() => handleDatePick(day)}
                  >
                    {day.getDate()}
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}
