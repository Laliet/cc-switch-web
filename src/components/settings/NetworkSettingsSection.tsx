import { RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { NetworkSettings } from "@/types";

interface NetworkSettingsSectionProps {
  value?: NetworkSettings;
  onChange: (value: NetworkSettings) => void;
}

const DEFAULT_NETWORK_SETTINGS: NetworkSettings = {
  githubMirrorBaseUrl: "",
};

export function NetworkSettingsSection({
  value,
  onChange,
}: NetworkSettingsSectionProps) {
  const { t } = useTranslation();
  const settings = { ...DEFAULT_NETWORK_SETTINGS, ...(value ?? {}) };

  const update = (patch: Partial<NetworkSettings>) => {
    onChange({ ...settings, ...patch });
  };

  return (
    <section className="space-y-4">
      <header className="space-y-1">
        <h3 className="text-sm font-medium">
          {t("settings.network.title", { defaultValue: "Network" })}
        </h3>
        <p className="text-xs text-muted-foreground">
          {t("settings.network.description", {
            defaultValue:
              "GitHub is used by default. Configure a mirror only when repository downloads are unstable.",
          })}
        </p>
      </header>

      <div className="space-y-2">
        <div className="flex items-center justify-between gap-3">
          <Label htmlFor="github-mirror-base-url">
            {t("settings.network.githubMirrorBaseUrl", {
              defaultValue: "GitHub mirror base URL",
            })}
          </Label>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => update({ githubMirrorBaseUrl: "" })}
          >
            <RotateCcw className="h-4 w-4" />
            {t("settings.network.useOrigin", {
              defaultValue: "Use GitHub",
            })}
          </Button>
        </div>
        <Input
          id="github-mirror-base-url"
          value={settings.githubMirrorBaseUrl}
          placeholder="https://ghproxy.net/"
          onChange={(event) =>
            update({ githubMirrorBaseUrl: event.target.value })
          }
        />
        <p className="text-xs text-muted-foreground">
          {t("settings.network.githubMirrorHint", {
            defaultValue:
              "Applied to GitHub ZIP archive downloads such as Skills repositories. Leave empty to use github.com directly.",
          })}
        </p>
      </div>
    </section>
  );
}
