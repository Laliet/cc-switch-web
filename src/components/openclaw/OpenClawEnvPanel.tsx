import { useEffect, useState } from "react";
import { Save } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import JsonEditor from "@/components/JsonEditor";
import { Button } from "@/components/ui/button";
import { useOpenClawEnv, useSaveOpenClawEnv } from "@/hooks/useOpenClaw";
import type { OpenClawEnvConfig } from "@/lib/api";
import { extractErrorMessage } from "@/utils/errorUtils";

interface OpenClawEnvPanelProps {
  enabled: boolean;
}

export function OpenClawEnvPanel({ enabled }: OpenClawEnvPanelProps) {
  const { t } = useTranslation();
  const query = useOpenClawEnv(enabled);
  const mutation = useSaveOpenClawEnv();
  const [value, setValue] = useState("{}");
  const [darkMode, setDarkMode] = useState(false);

  useEffect(() => {
    if (query.data) {
      setValue(JSON.stringify(query.data.value, null, 2));
    }
  }, [query.data]);

  useEffect(() => {
    const update = () =>
      setDarkMode(document.documentElement.classList.contains("dark"));
    update();
    const observer = new MutationObserver(update);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });
    return () => observer.disconnect();
  }, []);

  const save = async () => {
    if (!query.data) return;
    try {
      const parsed: unknown = JSON.parse(value);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        throw new Error(t("openclaw.config.objectRequired"));
      }
      await mutation.mutateAsync({
        env: parsed as OpenClawEnvConfig,
        expectedEtag: query.data.etag,
      });
      toast.success(t("openclaw.config.saved"));
    } catch (error) {
      toast.error(t("openclaw.config.saveFailed"), {
        description: extractErrorMessage(error),
      });
    }
  };

  if (query.isLoading) {
    return (
      <div className="grid min-h-64 place-items-center text-sm text-muted-foreground">
        {t("common.loading")}
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        <JsonEditor
          value={value}
          onChange={setValue}
          darkMode={darkMode}
          rows={22}
          showValidation={true}
          language="json"
        />
      </div>
      <div className="flex justify-end border-t px-5 py-3">
        <Button onClick={() => void save()} disabled={mutation.isPending}>
          <Save className="h-4 w-4" />
          {mutation.isPending ? t("common.saving") : t("common.save")}
        </Button>
      </div>
    </div>
  );
}
