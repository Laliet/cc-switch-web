import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pencil, Plus, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useDeleteModelPricing, useModelPricing } from "@/lib/query/usage";
import type { ModelPricing } from "@/types/usage";
import { PricingEditModal } from "./PricingEditModal";

const EMPTY_DRAFT: ModelPricing = {
  modelId: "",
  displayName: "",
  inputCostPerMillion: "0",
  outputCostPerMillion: "0",
  cacheReadCostPerMillion: "0",
  cacheCreationCostPerMillion: "0",
};

export function PricingConfigPanel() {
  const { t } = useTranslation();
  const query = useModelPricing();
  const remove = useDeleteModelPricing();
  const [filter, setFilter] = useState("");
  const [editingModel, setEditingModel] = useState<ModelPricing | null>(null);
  const [isAddingNew, setIsAddingNew] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);

  const rows = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    const data = query.data ?? [];
    if (!needle) return data.slice(0, 80);
    return data
      .filter(
        (row) =>
          row.modelId.toLowerCase().includes(needle) ||
          row.displayName.toLowerCase().includes(needle),
      )
      .slice(0, 80);
  }, [filter, query.data]);

  const openEditor = (row?: ModelPricing) => {
    setIsAddingNew(!row);
    setEditingModel(row ?? EMPTY_DRAFT);
  };

  const handleDelete = async () => {
    if (!deleteConfirm) return;
    try {
      await remove.mutateAsync(deleteConfirm);
      setDeleteConfirm(null);
      toast.success(
        t("usage.pricingDeleted", { defaultValue: "Pricing deleted" }),
      );
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <Input
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder="Filter model pricing"
          className="min-w-[220px] flex-1"
        />
        <Button onClick={() => openEditor()}>
          <Plus className="h-4 w-4" />
          {t("usage.addPricing", { defaultValue: "Add pricing" })}
        </Button>
      </div>

      <div className="rounded-lg border border-border-default bg-card">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>
                {t("usage.model", { defaultValue: "Model" })}
              </TableHead>
              <TableHead>
                {t("usage.displayName", { defaultValue: "Display name" })}
              </TableHead>
              <TableHead className="text-right">
                {t("usage.inputCost", { defaultValue: "Input" })}
              </TableHead>
              <TableHead className="text-right">
                {t("usage.outputCost", { defaultValue: "Output" })}
              </TableHead>
              <TableHead className="text-right">
                {t("usage.cacheReadCost", { defaultValue: "Cache read" })}
              </TableHead>
              <TableHead className="text-right">
                {t("usage.cacheCreationCost", {
                  defaultValue: "Cache create",
                })}
              </TableHead>
              <TableHead className="text-right">
                {t("common.actions", { defaultValue: "Actions" })}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((row) => (
              <TableRow key={row.modelId}>
                <TableCell className="max-w-[220px] truncate font-mono text-xs">
                  {row.modelId}
                </TableCell>
                <TableCell>{row.displayName}</TableCell>
                <TableCell className="text-right">
                  {row.inputCostPerMillion}
                </TableCell>
                <TableCell className="text-right">
                  {row.outputCostPerMillion}
                </TableCell>
                <TableCell className="text-right">
                  {row.cacheReadCostPerMillion}
                </TableCell>
                <TableCell className="text-right">
                  {row.cacheCreationCostPerMillion}
                </TableCell>
                <TableCell className="text-right">
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => openEditor(row)}
                    aria-label={`Edit pricing for ${row.modelId}`}
                    title="Edit pricing"
                  >
                    <Pencil className="h-4 w-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => setDeleteConfirm(row.modelId)}
                    aria-label={`Delete pricing for ${row.modelId}`}
                    title="Delete pricing"
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>

      {editingModel ? (
        <PricingEditModal
          open={Boolean(editingModel)}
          model={editingModel}
          isNew={isAddingNew}
          onClose={() => {
            setEditingModel(null);
            setIsAddingNew(false);
          }}
        />
      ) : null}

      <Dialog
        open={Boolean(deleteConfirm)}
        onOpenChange={(open) => !open && setDeleteConfirm(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {t("usage.deleteConfirmTitle", {
                defaultValue: "Delete pricing?",
              })}
            </DialogTitle>
            <DialogDescription>
              {t("usage.deleteConfirmDesc", {
                defaultValue:
                  "This removes the pricing record. Historical request logs are not deleted.",
              })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteConfirm(null)}>
              {t("common.cancel", { defaultValue: "Cancel" })}
            </Button>
            <Button
              variant="destructive"
              onClick={() => void handleDelete()}
              disabled={remove.isPending}
            >
              {remove.isPending
                ? t("common.deleting", { defaultValue: "Deleting..." })
                : t("common.delete", { defaultValue: "Delete" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
