import type { AppId } from "@/lib/api";

export interface AppDefinition {
  id: AppId;
  labelKey: `apps.${AppId}`;
}

export const PROVIDER_APPS = [
  "claude",
  "codex",
  "gemini",
  "opencode",
  "omo",
  "omo-slim",
] as const satisfies readonly AppId[];

export const PROMPT_APPS = [
  "claude",
  "codex",
  "gemini",
  "opencode",
] as const satisfies readonly AppId[];

export const MCP_APPS = [
  "claude",
  "codex",
  "gemini",
  "opencode",
  "omo",
  "omo-slim",
] as const satisfies readonly AppId[];

export const SKILLS_APPS = [
  "claude",
  "codex",
  "gemini",
  "opencode",
  "omo",
  "omo-slim",
] as const satisfies readonly AppId[];

export const DIRECTORY_APPS = [
  "claude",
  "codex",
  "gemini",
  "opencode",
] as const satisfies readonly AppId[];

export const USAGE_APPS = [
  "claude",
  "codex",
  "gemini",
  "opencode",
] as const satisfies readonly AppId[];

export type PromptAppId = (typeof PROMPT_APPS)[number];
export type McpAppId = (typeof MCP_APPS)[number];
export type SkillsAppId = (typeof SKILLS_APPS)[number];
export type DirectoryAppId = (typeof DIRECTORY_APPS)[number];
export type UsageAppId = (typeof USAGE_APPS)[number];

export const SWITCHER_APPS: AppDefinition[] = PROVIDER_APPS.map((id) => ({
  id,
  labelKey: `apps.${id}` as const,
}));

const providerAppSet = new Set<AppId>(PROVIDER_APPS);
const promptAppSet = new Set<AppId>(PROMPT_APPS);
const mcpAppSet = new Set<AppId>(MCP_APPS);
const skillsAppSet = new Set<AppId>(SKILLS_APPS);
const directoryAppSet = new Set<AppId>(DIRECTORY_APPS);
const usageAppSet = new Set<AppId>(USAGE_APPS);

export function isProviderApp(value: unknown): value is AppId {
  return typeof value === "string" && providerAppSet.has(value as AppId);
}

export function isPromptApp(appId: AppId): appId is PromptAppId {
  return promptAppSet.has(appId);
}

export function isMcpApp(appId: AppId): appId is McpAppId {
  return mcpAppSet.has(appId);
}

export function isSkillsApp(appId: AppId): appId is SkillsAppId {
  return skillsAppSet.has(appId);
}

export function isDirectoryApp(appId: AppId): appId is DirectoryAppId {
  return directoryAppSet.has(appId);
}

export function isUsageApp(appId: AppId): appId is UsageAppId {
  return usageAppSet.has(appId);
}
