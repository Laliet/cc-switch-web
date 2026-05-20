import anthropicIcon from "@/assets/provider-icons/anthropic.svg";
import openaiIcon from "@/assets/provider-icons/openai.svg";
import googleIcon from "@/assets/provider-icons/google.svg";
import geminiIcon from "@/assets/provider-icons/gemini.svg";
import deepseekIcon from "@/assets/provider-icons/deepseek.svg";
import bailianIcon from "@/assets/provider-icons/bailian.svg";
import alibabaIcon from "@/assets/provider-icons/alibaba.svg";
import qwenIcon from "@/assets/provider-icons/qwen.svg";
import kimiIcon from "@/assets/provider-icons/kimi.svg";
import minimaxIcon from "@/assets/provider-icons/minimax.svg";
import doubaoIcon from "@/assets/provider-icons/doubao.svg";
import huoshanIcon from "@/assets/provider-icons/huoshan.png";
import byteplusIcon from "@/assets/provider-icons/byteplus.png";
import aihubmixIcon from "@/assets/provider-icons/aihubmix-color.svg";
import packycodeIcon from "@/assets/provider-icons/packycode.svg";
import aicodemirrorIcon from "@/assets/provider-icons/aicodemirror.svg";
import zhipuIcon from "@/assets/provider-icons/zhipu.svg";
import chatglmIcon from "@/assets/provider-icons/chatglm.svg";
import modelscopeIcon from "@/assets/provider-icons/modelscope-color.svg";
import azureIcon from "@/assets/provider-icons/azure.svg";
import copilotIcon from "@/assets/provider-icons/githubcopilot.svg";
import awsIcon from "@/assets/provider-icons/aws.svg";
import longcatIcon from "@/assets/provider-icons/longcat-color.svg";
import opencodeIcon from "@/assets/provider-icons/opencode-logo-light.svg";
import { cn } from "@/lib/utils";

const PROVIDER_ICON_MATCHERS: Array<[RegExp, string]> = [
  [/claude|anthropic/i, anthropicIcon],
  [/openai|chatgpt|codex/i, openaiIcon],
  [/gemini/i, geminiIcon],
  [/google/i, googleIcon],
  [/deepseek/i, deepseekIcon],
  [/qwen|dashscope|bailian|aliyun|alibaba/i, qwenIcon],
  [/百炼|通义/i, bailianIcon],
  [/阿里/i, alibabaIcon],
  [/kimi|moonshot/i, kimiIcon],
  [/zhipu|z\.ai|chatglm|glm/i, zhipuIcon],
  [/智谱/i, chatglmIcon],
  [/minimax/i, minimaxIcon],
  [/doubao|bytedance|volc|ark/i, doubaoIcon],
  [/火山|方舟/i, huoshanIcon],
  [/byteplus/i, byteplusIcon],
  [/aihubmix/i, aihubmixIcon],
  [/packy/i, packycodeIcon],
  [/aicodemirror/i, aicodemirrorIcon],
  [/modelscope/i, modelscopeIcon],
  [/azure/i, azureIcon],
  [/copilot/i, copilotIcon],
  [/aws|bedrock/i, awsIcon],
  [/longcat/i, longcatIcon],
  [/opencode|omo|oh-my-opencode/i, opencodeIcon],
];

interface ProviderIconProps {
  name: string;
  websiteUrl?: string;
  size?: number;
  className?: string;
  showFallback?: boolean;
}

export function resolveProviderIcon(
  name: string,
  websiteUrl?: string,
): string | undefined {
  const text = `${name} ${websiteUrl ?? ""}`;
  return PROVIDER_ICON_MATCHERS.find(([pattern]) => pattern.test(text))?.[1];
}

export function ProviderIcon({
  name,
  websiteUrl,
  size = 28,
  className,
  showFallback = true,
}: ProviderIconProps) {
  const src = resolveProviderIcon(name, websiteUrl);

  if (src) {
    return (
      <img
        src={src}
        alt=""
        aria-hidden="true"
        className={cn("shrink-0 object-contain", className)}
        style={{ width: size, height: size }}
        loading="lazy"
      />
    );
  }

  if (!showFallback) {
    return null;
  }

  const initials = name
    .split(/\s+/)
    .filter(Boolean)
    .map((part) => part[0])
    .join("")
    .toUpperCase()
    .slice(0, 2);

  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-md bg-muted text-xs font-semibold text-muted-foreground",
        className,
      )}
      style={{ width: size, height: size }}
      aria-hidden="true"
    >
      {initials || "AI"}
    </span>
  );
}
