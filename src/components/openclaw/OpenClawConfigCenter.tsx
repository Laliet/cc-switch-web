import { useState } from "react";
import {
  AlertTriangle,
  Bot,
  FileJson2,
  KeyRound,
  RefreshCw,
  Settings2,
  ShieldCheck,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useOpenClawStatus } from "@/hooks/useOpenClaw";
import { OpenClawAgentsPanel } from "./OpenClawAgentsPanel";
import { OpenClawEnvPanel } from "./OpenClawEnvPanel";
import { OpenClawReconciliationPanel } from "./OpenClawReconciliationPanel";
import { OpenClawRawConfigPanel } from "./OpenClawRawConfigPanel";
import { OpenClawToolsPanel } from "./OpenClawToolsPanel";

interface OpenClawConfigCenterProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function OpenClawConfigCenter({
  open,
  onOpenChange,
}: OpenClawConfigCenterProps) {
  const { t } = useTranslation();
  const [tab, setTab] = useState("agents");
  const statusQuery = useOpenClawStatus(open);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[min(860px,92vh)] max-w-[min(1040px,96vw)] flex-col gap-0 p-0">
        <DialogHeader className="border-b px-6 py-4">
          <DialogTitle className="flex items-center gap-2">
            <Settings2 className="h-5 w-5" />
            {t("openclaw.config.title")}
          </DialogTitle>
          <DialogDescription>
            {t("openclaw.config.path", { path: "~/.openclaw/openclaw.json" })}
          </DialogDescription>
        </DialogHeader>

        {statusQuery.data?.warnings.length ? (
          <div className="px-5 pt-4">
            <Alert className="border-amber-500/30 bg-amber-500/5">
              <AlertTriangle className="absolute left-4 top-3.5 h-4 w-4 text-amber-600" />
              <AlertDescription className="pl-6">
                <ul className="space-y-1 text-xs">
                  {statusQuery.data.warnings.map((warning) => (
                    <li key={`${warning.code}:${warning.path ?? ""}`}>
                      {warning.message}
                      {warning.path ? ` (${warning.path})` : ""}
                    </li>
                  ))}
                </ul>
              </AlertDescription>
            </Alert>
          </div>
        ) : null}

        <Tabs
          value={tab}
          onValueChange={setTab}
          className="flex min-h-0 flex-1 flex-col"
        >
          <TabsList className="mx-5 mt-4 grid h-auto w-auto grid-cols-2 sm:grid-cols-5">
            <TabsTrigger value="agents" className="gap-2">
              <Bot className="h-4 w-4" />
              {t("openclaw.config.tabs.agents")}
            </TabsTrigger>
            <TabsTrigger value="env" className="gap-2">
              <KeyRound className="h-4 w-4" />
              {t("openclaw.config.tabs.env")}
            </TabsTrigger>
            <TabsTrigger value="tools" className="gap-2">
              <ShieldCheck className="h-4 w-4" />
              {t("openclaw.config.tabs.tools")}
            </TabsTrigger>
            <TabsTrigger value="reconcile" className="gap-2">
              <RefreshCw className="h-4 w-4" />
              {t("openclaw.config.tabs.reconcile")}
            </TabsTrigger>
            <TabsTrigger value="raw" className="gap-2">
              <FileJson2 className="h-4 w-4" />
              {t("openclaw.config.tabs.raw")}
            </TabsTrigger>
          </TabsList>

          <TabsContent
            value="agents"
            className="mt-3 min-h-0 flex-1 overflow-hidden"
          >
            <OpenClawAgentsPanel enabled={open && tab === "agents"} />
          </TabsContent>
          <TabsContent
            value="env"
            className="mt-3 min-h-0 flex-1 overflow-hidden"
          >
            <OpenClawEnvPanel enabled={open && tab === "env"} />
          </TabsContent>
          <TabsContent
            value="tools"
            className="mt-3 min-h-0 flex-1 overflow-hidden"
          >
            <OpenClawToolsPanel enabled={open && tab === "tools"} />
          </TabsContent>
          <TabsContent
            value="reconcile"
            className="mt-3 min-h-0 flex-1 overflow-hidden"
          >
            <OpenClawReconciliationPanel
              enabled={open && tab === "reconcile"}
            />
          </TabsContent>
          <TabsContent
            value="raw"
            className="mt-3 min-h-0 flex-1 overflow-hidden"
          >
            <OpenClawRawConfigPanel enabled={open && tab === "raw"} />
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}
