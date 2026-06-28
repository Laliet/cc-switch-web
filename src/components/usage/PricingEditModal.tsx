import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Save } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useUpdateModelPricing } from "@/lib/query/usage";
import { isNonNegativeDecimalString, type ModelPricing } from "@/types/usage";

interface PricingEditModalProps {
  open: boolean;
  model: ModelPricing;
  isNew?: boolean;
  onClose: () => void;
}

export function PricingEditModal({
  open,
  model,
  isNew = false,
  onClose,
}: PricingEditModalProps) {
  const { t } = useTranslation();
  const updatePricing = useUpdateModelPricing();
  const [formData, setFormData] = useState({
    modelId: model.modelId,
    displayName: model.displayName,
    inputCost: model.inputCostPerMillion,
    outputCost: model.outputCostPerMillion,
    cacheReadCost: model.cacheReadCostPerMillion,
    cacheCreationCost: model.cacheCreationCostPerMillion,
  });

  useEffect(() => {
    if (!open) return;
    setFormData({
      modelId: model.modelId,
      displayName: model.displayName,
      inputCost: model.inputCostPerMillion,
      outputCost: model.outputCostPerMillion,
      cacheReadCost: model.cacheReadCostPerMillion,
      cacheCreationCost: model.cacheCreationCostPerMillion,
    });
  }, [model, open]);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (isNew && !formData.modelId.trim()) {
      toast.error(
        t("usage.modelIdRequired", { defaultValue: "Model ID is required" }),
      );
      return;
    }
    if (!formData.displayName.trim()) {
      toast.error(
        t("usage.displayNameRequired", {
          defaultValue: "Display name is required",
        }),
      );
      return;
    }
    const priceFields = [
      formData.inputCost,
      formData.outputCost,
      formData.cacheReadCost,
      formData.cacheCreationCost,
    ];
    if (priceFields.some((value) => !isNonNegativeDecimalString(value))) {
      toast.error(
        t("usage.invalidPrice", {
          defaultValue: "Prices must be non-negative numbers",
        }),
      );
      return;
    }

    try {
      await updatePricing.mutateAsync({
        modelId: isNew ? formData.modelId.trim() : model.modelId,
        displayName: formData.displayName.trim(),
        inputCostPerMillion: formData.inputCost.trim(),
        outputCostPerMillion: formData.outputCost.trim(),
        cacheReadCostPerMillion: formData.cacheReadCost.trim(),
        cacheCreationCostPerMillion: formData.cacheCreationCost.trim(),
      });
      toast.success(
        isNew
          ? t("usage.pricingAdded", { defaultValue: "Pricing added" })
          : t("usage.pricingUpdated", { defaultValue: "Pricing updated" }),
      );
      onClose();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>
            {isNew
              ? t("usage.addPricing", { defaultValue: "Add pricing" })
              : t("usage.editPricing", { defaultValue: "Edit pricing" })}
          </DialogTitle>
        </DialogHeader>
        <form
          id="usage-pricing-form"
          onSubmit={handleSubmit}
          className="grid gap-3 px-6 py-5 sm:grid-cols-2"
        >
          <Field
            id="usage-pricing-model-id"
            label={t("usage.modelId", { defaultValue: "Model ID" })}
            value={formData.modelId}
            disabled={!isNew}
            onChange={(value) => setFormData({ ...formData, modelId: value })}
            placeholder="claude-3-5-sonnet-20241022"
          />
          <Field
            id="usage-pricing-display-name"
            label={t("usage.displayName", { defaultValue: "Display name" })}
            value={formData.displayName}
            onChange={(value) =>
              setFormData({ ...formData, displayName: value })
            }
            placeholder="Claude 3.5 Sonnet"
          />
          <Field
            id="usage-pricing-input"
            label={t("usage.inputCostPerMillion", {
              defaultValue: "Input cost / 1M tokens",
            })}
            value={formData.inputCost}
            onChange={(value) => setFormData({ ...formData, inputCost: value })}
            numeric
          />
          <Field
            id="usage-pricing-output"
            label={t("usage.outputCostPerMillion", {
              defaultValue: "Output cost / 1M tokens",
            })}
            value={formData.outputCost}
            onChange={(value) =>
              setFormData({ ...formData, outputCost: value })
            }
            numeric
          />
          <Field
            id="usage-pricing-cache-read"
            label={t("usage.cacheReadCostPerMillion", {
              defaultValue: "Cache read cost / 1M tokens",
            })}
            value={formData.cacheReadCost}
            onChange={(value) =>
              setFormData({ ...formData, cacheReadCost: value })
            }
            numeric
          />
          <Field
            id="usage-pricing-cache-create"
            label={t("usage.cacheCreationCostPerMillion", {
              defaultValue: "Cache create cost / 1M tokens",
            })}
            value={formData.cacheCreationCost}
            onChange={(value) =>
              setFormData({ ...formData, cacheCreationCost: value })
            }
            numeric
          />
        </form>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("common.cancel", { defaultValue: "Cancel" })}
          </Button>
          <Button
            type="submit"
            form="usage-pricing-form"
            disabled={updatePricing.isPending}
          >
            {isNew ? (
              <Plus className="mr-2 h-4 w-4" />
            ) : (
              <Save className="mr-2 h-4 w-4" />
            )}
            {updatePricing.isPending
              ? t("common.saving", { defaultValue: "Saving..." })
              : isNew
                ? t("common.add", { defaultValue: "Add" })
                : t("common.save", { defaultValue: "Save" })}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Field({
  id,
  label,
  value,
  onChange,
  placeholder,
  disabled,
  numeric,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  numeric?: boolean;
}) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        type={numeric ? "number" : "text"}
        step={numeric ? "0.01" : undefined}
        min={numeric ? "0" : undefined}
        inputMode={numeric ? "decimal" : undefined}
        value={value}
        disabled={disabled}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        required
      />
    </div>
  );
}
