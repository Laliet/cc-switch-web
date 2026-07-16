import { useEffect, useState } from "react";
import { Plus, Save, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useOpenClawTools, useSaveOpenClawTools } from "@/hooks/useOpenClaw";
import type { OpenClawToolsConfig } from "@/lib/api";
import { extractErrorMessage } from "@/utils/errorUtils";

const UNSET = "__openclaw_tools_unset__";
const PROFILES = ["minimal", "coding", "messaging", "full"] as const;

interface OpenClawToolsPanelProps {
  enabled: boolean;
}

export function OpenClawToolsPanel({ enabled }: OpenClawToolsPanelProps) {
  const { t } = useTranslation();
  const query = useOpenClawTools(enabled);
  const mutation = useSaveOpenClawTools();
  const [source, setSource] = useState<OpenClawToolsConfig>({});
  const [profile, setProfile] = useState("");
  const [allow, setAllow] = useState<string[]>([]);
  const [deny, setDeny] = useState<string[]>([]);

  useEffect(() => {
    if (!query.data) return;
    setSource(query.data.value);
    setProfile(query.data.value.profile ?? "");
    setAllow(query.data.value.allow ?? []);
    setDeny(query.data.value.deny ?? []);
  }, [query.data]);

  const save = async () => {
    if (!query.data) return;
    try {
      await mutation.mutateAsync({
        tools: {
          ...source,
          profile: profile || undefined,
          allow: allow.map((item) => item.trim()).filter(Boolean),
          deny: deny.map((item) => item.trim()).filter(Boolean),
        },
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

  const unsupported =
    profile && !PROFILES.includes(profile as (typeof PROFILES)[number]);

  return (
    <div className="min-h-0 overflow-y-auto">
      <section className="border-b px-5 py-4">
        <Label>{t("openclaw.config.toolsProfile")}</Label>
        <Select
          value={profile || UNSET}
          onValueChange={(value) => setProfile(value === UNSET ? "" : value)}
        >
          <SelectTrigger className="mt-1.5 w-full sm:w-72">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={UNSET}>{t("common.none")}</SelectItem>
            {unsupported ? (
              <SelectItem value={profile}>{profile}</SelectItem>
            ) : null}
            {PROFILES.map((value) => (
              <SelectItem key={value} value={value}>
                {t(`openclaw.config.profile.${value}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </section>

      <ListEditor
        title={t("openclaw.config.allow")}
        values={allow}
        onChange={setAllow}
      />
      <ListEditor
        title={t("openclaw.config.deny")}
        values={deny}
        onChange={setDeny}
      />

      <div className="sticky bottom-0 flex justify-end border-t bg-background px-5 py-3">
        <Button onClick={() => void save()} disabled={mutation.isPending}>
          <Save className="h-4 w-4" />
          {mutation.isPending ? t("common.saving") : t("common.save")}
        </Button>
      </div>
    </div>
  );
}

function ListEditor({
  title,
  values,
  onChange,
}: {
  title: string;
  values: string[];
  onChange: (values: string[]) => void;
}) {
  const { t } = useTranslation();
  return (
    <section className="border-b px-5 py-4">
      <div className="mb-3 flex items-center justify-between gap-2">
        <h3 className="text-sm font-medium">{title}</h3>
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => onChange([...values, ""])}
        >
          <Plus className="h-4 w-4" />
          {t("common.add")}
        </Button>
      </div>
      <div className="space-y-2">
        {values.map((value, index) => (
          <div className="flex items-center gap-2" key={index}>
            <Input
              className="min-w-0 flex-1 font-mono text-xs"
              value={value}
              onChange={(event) =>
                onChange(
                  values.map((item, itemIndex) =>
                    itemIndex === index ? event.target.value : item,
                  ),
                )
              }
            />
            <Button
              type="button"
              size="icon"
              variant="ghost"
              title={t("common.delete")}
              onClick={() =>
                onChange(values.filter((_, itemIndex) => itemIndex !== index))
              }
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
        ))}
      </div>
    </section>
  );
}
