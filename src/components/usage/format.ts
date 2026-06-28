export function formatUsd(value: string | number): string {
  const number = typeof value === "number" ? value : Number.parseFloat(value);
  if (!Number.isFinite(number)) return "$0.000000";
  if (number === 0) return "$0";
  if (number < 0.0001) return `$${number.toFixed(6)}`;
  return `$${number.toFixed(4)}`;
}

export function formatNumber(value: number): string {
  return new Intl.NumberFormat().format(value);
}

export function formatPercent(value: number): string {
  return `${value.toFixed(1)}%`;
}

export function formatDateTime(ms: number): string {
  if (!Number.isFinite(ms)) return "-";
  return new Date(ms).toLocaleString();
}

export function getLocaleFromLanguage(language?: string): string {
  if (!language) return "en-US";
  if (language.startsWith("zh")) return "zh-CN";
  if (language.startsWith("ja")) return "ja-JP";
  return "en-US";
}

export function parseFiniteNumber(value: string | number | null | undefined) {
  if (value === null || value === undefined) return null;
  const parsed = typeof value === "number" ? value : Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : null;
}

export function compactDate(value: string): string {
  if (!value) return "-";
  if (/^\d{4}-\d{2}-\d{2}$/.test(value)) return value.slice(5);
  return value.replace("T", " ").slice(5, 16);
}

export function statusTone(statusCode: number): string {
  if (statusCode >= 200 && statusCode < 300) {
    return "bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-950/40 dark:text-emerald-300 dark:border-emerald-900";
  }
  if (statusCode >= 500 || statusCode === 0) {
    return "bg-red-50 text-red-700 border-red-200 dark:bg-red-950/40 dark:text-red-300 dark:border-red-900";
  }
  return "bg-amber-50 text-amber-700 border-amber-200 dark:bg-amber-950/40 dark:text-amber-300 dark:border-amber-900";
}
