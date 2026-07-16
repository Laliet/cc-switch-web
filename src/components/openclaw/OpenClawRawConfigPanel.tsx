import { useEffect, useState } from "react";
import { Save } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import JsonEditor from "@/components/JsonEditor";
import { Button } from "@/components/ui/button";
import {
  useOpenClawRawConfig,
  useSaveOpenClawRawConfig,
} from "@/hooks/useOpenClaw";
import { extractErrorMessage } from "@/utils/errorUtils";

interface OpenClawRawConfigPanelProps {
  enabled: boolean;
}

export function OpenClawRawConfigPanel({
  enabled,
}: OpenClawRawConfigPanelProps) {
  const { t } = useTranslation();
  const query = useOpenClawRawConfig(enabled);
  const mutation = useSaveOpenClawRawConfig();
  const [value, setValue] = useState("");
  const [darkMode, setDarkMode] = useState(false);

  useEffect(() => {
    if (query.data) setValue(query.data.value);
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
      await mutation.mutateAsync({
        source: value,
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
          rows={24}
          showValidation={false}
          language="javascript"
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
