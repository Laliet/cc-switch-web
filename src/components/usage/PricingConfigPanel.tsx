import { useMemo, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  useDeleteModelPricing,
  useModelPricing,
  useUpdateModelPricing,
} from "@/lib/query/usage";
import type { ModelPricing } from "@/types/usage";

const EMPTY_DRAFT: ModelPricing = {
  modelId: "",
  displayName: "",
  inputCostPerMillion: "0",
  outputCostPerMillion: "0",
  cacheReadCostPerMillion: "0",
  cacheCreationCostPerMillion: "0",
};

export function PricingConfigPanel() {
  const query = useModelPricing();
  const update = useUpdateModelPricing();
  const remove = useDeleteModelPricing();
  const [filter, setFilter] = useState("");
  const [draft, setDraft] = useState<ModelPricing>(EMPTY_DRAFT);

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

  const saveDraft = async () => {
    if (!draft.modelId.trim() || !draft.displayName.trim()) {
      toast.error("Model ID and display name are required");
      return;
    }
    try {
      const updated = await update.mutateAsync({
        ...draft,
        modelId: draft.modelId.trim(),
        displayName: draft.displayName.trim(),
      });
      setDraft(EMPTY_DRAFT);
      toast.success(`Pricing saved, ${updated} historical logs backfilled`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Pricing save failed");
    }
  };

  return (
    <div className="space-y-4">
      <div className="grid gap-2 md:grid-cols-7">
        <Input
          value={draft.modelId}
          onChange={(event) => setDraft({ ...draft, modelId: event.target.value })}
          placeholder="model id"
          className="md:col-span-2"
        />
        <Input
          value={draft.displayName}
          onChange={(event) =>
            setDraft({ ...draft, displayName: event.target.value })
          }
          placeholder="display name"
          className="md:col-span-2"
        />
        <Input
          value={draft.inputCostPerMillion}
          onChange={(event) =>
            setDraft({ ...draft, inputCostPerMillion: event.target.value })
          }
          placeholder="input"
        />
        <Input
          value={draft.outputCostPerMillion}
          onChange={(event) =>
            setDraft({ ...draft, outputCostPerMillion: event.target.value })
          }
          placeholder="output"
        />
        <Button onClick={() => void saveDraft()} disabled={update.isPending}>
          Save
        </Button>
        <Input
          value={draft.cacheReadCostPerMillion}
          onChange={(event) =>
            setDraft({ ...draft, cacheReadCostPerMillion: event.target.value })
          }
          placeholder="cache read"
        />
        <Input
          value={draft.cacheCreationCostPerMillion}
          onChange={(event) =>
            setDraft({
              ...draft,
              cacheCreationCostPerMillion: event.target.value,
            })
          }
          placeholder="cache create"
        />
      </div>

      <Input
        value={filter}
        onChange={(event) => setFilter(event.target.value)}
        placeholder="Filter model pricing"
      />

      <div className="rounded-lg border border-border-default bg-card">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Model</TableHead>
              <TableHead>Name</TableHead>
              <TableHead className="text-right">Input</TableHead>
              <TableHead className="text-right">Output</TableHead>
              <TableHead className="text-right">Cache read</TableHead>
              <TableHead className="text-right">Cache create</TableHead>
              <TableHead className="text-right">Action</TableHead>
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
                    size="sm"
                    onClick={() => setDraft(row)}
                  >
                    Edit
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => void remove.mutateAsync(row.modelId)}
                  >
                    Delete
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}
