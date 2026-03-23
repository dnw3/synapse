import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchJSON, postJSON, putJSON, deleteJSON } from "../../lib/api";
import { useToast } from "../../components/ui/toast";
import type { ScheduleEntry, ScheduleRunEntry } from "../../types/dashboard";

export const schedulesKeys = {
  all: ["schedules"] as const,
  list: () => [...schedulesKeys.all, "list"] as const,
  runs: (name: string) => [...schedulesKeys.all, "runs", name] as const,
};

export function useSchedules() {
  return useQuery({
    queryKey: schedulesKeys.list(),
    queryFn: () => fetchJSON<ScheduleEntry[]>("/schedules"),
  });
}

export function useCreateSchedule() {
  const qc = useQueryClient();
  const { toast } = useToast();
  return useMutation({
    mutationFn: (schedule: Partial<ScheduleEntry>) =>
      postJSON<ScheduleEntry>("/schedules", schedule),
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
      putJSON<ScheduleEntry>(`/schedules/${encodeURIComponent(name)}`, schedule),
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
      postJSON<{ ok: boolean }>(`/schedules/${encodeURIComponent(name)}/trigger`),
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
      postJSON<{ enabled: boolean }>(`/schedules/${encodeURIComponent(name)}/toggle`),
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
      fetchJSON<ScheduleRunEntry[]>(`/schedules/${encodeURIComponent(name)}/runs`),
    enabled: !!name,
  });
}
