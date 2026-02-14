import type { AppId } from "@/lib/api";

export type UpcomingAppId = "opencode" | "omo";

export interface SupportedAppDefinition {
  id: AppId;
  kind: "supported";
  labelKey: `apps.${AppId}`;
}

export interface UpcomingAppDefinition {
  id: UpcomingAppId;
  kind: "upcoming";
  labelKey: `apps.${UpcomingAppId}`;
}

export type AppDefinition = SupportedAppDefinition | UpcomingAppDefinition;

export const SUPPORTED_APPS: SupportedAppDefinition[] = [
  { id: "claude", kind: "supported", labelKey: "apps.claude" },
  { id: "codex", kind: "supported", labelKey: "apps.codex" },
  { id: "gemini", kind: "supported", labelKey: "apps.gemini" },
];

export const UPCOMING_APPS: UpcomingAppDefinition[] = [
  { id: "opencode", kind: "upcoming", labelKey: "apps.opencode" },
  { id: "omo", kind: "upcoming", labelKey: "apps.omo" },
];

export const SWITCHER_APPS: AppDefinition[] = [...SUPPORTED_APPS, ...UPCOMING_APPS];
