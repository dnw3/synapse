import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { TraceSubView, TraceRecord, TraceListParams } from "./types";
import { fetchTraces, fetchTraceDetail } from "./api";
import { TraceList } from "./TraceList";
import { TraceDetail } from "./TraceDetail";
import { LoadingSpinner } from "../shared";

interface TracesPageProps {
  initialTraceId?: string;
  initialView?: TraceSubView;
}

type PageMode =
  | { mode: "list" }
  | { mode: "detail"; requestId: string; subView: TraceSubView };

export function TracesPage({ initialTraceId, initialView }: TracesPageProps) {
  const { t } = useTranslation();

  const [page, setPage] = useState<PageMode>(
    initialTraceId
      ? { mode: "detail", requestId: initialTraceId, subView: initialView ?? "overview" }
      : { mode: "list" }
  );

  // List state
  const [traces, setTraces] = useState<TraceRecord[]>([]);
  const [listLoading, setListLoading] = useState(false);
  const [filters, setFilters] = useState<TraceListParams>({});

  // Detail state
  const [detailTrace, setDetailTrace] = useState<TraceRecord | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Fetch list
  const loadTraces = useCallback(async () => {
    setListLoading(true);
    try {
      const res = await fetchTraces(filters);
      setTraces(res.traces);
    } catch {
      // silently fail — user can retry
    } finally {
      setListLoading(false);
    }
  }, [filters]);

  // Fetch detail
  const loadDetail = useCallback(async (requestId: string) => {
    setDetailLoading(true);
    try {
      const trace = await fetchTraceDetail(requestId);
      setDetailTrace(trace);
    } catch {
      setDetailTrace(null);
    } finally {
      setDetailLoading(false);
    }
  }, []);

  // Poll list every 5s when in list mode
  useEffect(() => {
    if (page.mode !== "list") return;
    loadTraces();
    pollRef.current = setInterval(loadTraces, 5000);
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [page.mode, loadTraces]);

  // Load detail when entering detail mode
  const detailRequestId = page.mode === "detail" ? page.requestId : null;
  useEffect(() => {
    if (detailRequestId) {
      loadDetail(detailRequestId);
    }
  }, [detailRequestId, loadDetail]);

  const handleSelectTrace = useCallback((requestId: string) => {
    setPage({ mode: "detail", requestId, subView: "overview" });
  }, []);

  const handleBack = useCallback(() => {
    setDetailTrace(null);
    setPage({ mode: "list" });
  }, []);

  const handleSubViewChange = useCallback((view: TraceSubView) => {
    setPage((prev) =>
      prev.mode === "detail" ? { ...prev, subView: view } : prev
    );
  }, []);

  const handleNavigateToTrace = useCallback((requestId: string) => {
    setPage({ mode: "detail", requestId, subView: "overview" });
  }, []);

  const handleFilterChange = useCallback((newFilters: TraceListParams) => {
    setFilters(newFilters);
  }, []);

  const handleRefresh = useCallback(() => {
    loadTraces();
  }, [loadTraces]);

  if (page.mode === "list") {
    return (
      <TraceList
        traces={traces}
        loading={listLoading}
        onSelectTrace={handleSelectTrace}
        filters={filters}
        onFilterChange={handleFilterChange}
        onRefresh={handleRefresh}
      />
    );
  }

  if (detailLoading && !detailTrace) {
    return <LoadingSpinner />;
  }

  if (!detailTrace) {
    return (
      <div className="flex flex-col items-center justify-center py-16 gap-3 text-[var(--text-tertiary)]">
        <span className="text-[16px] font-semibold text-[var(--text-secondary)]">
          {t("traces.error.notFound")}
        </span>
        <button
          onClick={handleBack}
          className="text-[12px] text-[var(--accent)] hover:underline cursor-pointer"
        >
          {t("traces.detail.backToList")}
        </button>
      </div>
    );
  }

  return (
    <TraceDetail
      trace={detailTrace}
      subView={page.subView}
      onSubViewChange={handleSubViewChange}
      onBack={handleBack}
      onNavigateToTrace={handleNavigateToTrace}
    />
  );
}
