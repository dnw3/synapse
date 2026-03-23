import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { fetchJSON, postJSON, putJSON, deleteJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import { ScheduleEntrySchema, ScheduleRunEntrySchema, OkResponseSchema } from "../../schemas/dashboard";
import type { ScheduleEntry } from "../../types/dashboard";

export const schedulesKeys = {
  all: ["schedules"] as const,
  list: () => [...schedulesKeys.all, "list"] as const,
  runs: (name: string) => [...schedulesKeys.all, "runs", name] as const,
};

export function useSchedules() {
  return useQuery({
    queryKey: schedulesKeys.list(),
    queryFn: () => fetchJSON("/schedules", z.array(ScheduleEntrySchema)),
  });
}

export function useCreateSchedule() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (schedule: Partial<ScheduleEntry>) =>
      postJSON("/schedules", schedule, ScheduleEntrySchema),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: schedulesKeys.all });
      toast({ variant: "success", title: "Schedule created" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to create schedule", description: err.message });
    },
  });
}

export function useUpdateSchedule() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: ({ name, schedule }: { name: string; schedule: Partial<ScheduleEntry> }) =>
      putJSON(`/schedules/${encodeURIComponent(name)}`, schedule, ScheduleEntrySchema),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: schedulesKeys.all });
      toast({ variant: "success", title: "Schedule updated" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to update schedule", description: err.message });
    },
  });
}

export function useDeleteSchedule() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) => deleteJSON(`/schedules/${encodeURIComponent(name)}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: schedulesKeys.all });
      toast({ variant: "success", title: "Schedule deleted" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to delete schedule", description: err.message });
    },
  });
}

export function useTriggerSchedule() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) =>
      postJSON(`/schedules/${encodeURIComponent(name)}/trigger`, undefined, OkResponseSchema),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: schedulesKeys.all });
      toast({ variant: "success", title: "Schedule triggered" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to trigger schedule", description: err.message });
    },
  });
}

export function useToggleSchedule() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (name: string) =>
      postJSON(`/schedules/${encodeURIComponent(name)}/toggle`, undefined, z.object({ enabled: z.boolean() })),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: schedulesKeys.all });
      toast({ variant: "success", title: "Schedule toggled" });
    },
    onError: (err: Error) => {
      toast({ variant: "error", title: "Failed to toggle schedule", description: err.message });
    },
  });
}

export function useScheduleRuns(name: string) {
  return useQuery({
    queryKey: schedulesKeys.runs(name),
    queryFn: () =>
      fetchJSON(`/schedules/${encodeURIComponent(name)}/runs`, z.array(ScheduleRunEntrySchema)),
    enabled: !!name,
  });
}
