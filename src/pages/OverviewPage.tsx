import { useEffect, useState } from "react";
import { api, formatBytes, formatDate } from "../api";
import type { Page } from "../App";
import type { ScanSummary } from "../types";

interface Props {
  sessionId: number;
  onNavigate: (page: Page) => void;
}

export default function OverviewPage({ sessionId, onNavigate }: Props) {
  const [summary, setSummary] = useState<ScanSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setSummary(null);
    api.scanSummary(sessionId).then(setSummary).catch((e) => setError(String(e)));
  }, [sessionId]);

  if (error) return <p className="form-error">{error}</p>;
  if (!summary) return <p>Loading…</p>;

  const s = summary.session;

  return (
    <section>
      <div className="page-header">
        <h2>Overview</h2>
        <div className="meta">
          Scan #{s.id} · started {formatDate(s.started_at_ms)}
          {s.finished_at_ms ? ` · finished ${formatDate(s.finished_at_ms)}` : ""}
        </div>
        <span className={`badge status-${s.status}`}>{s.status}</span>
      </div>
      {s.status === "cancelled" && (
        <p className="notice notice-warn">
          This scan was cancelled. Partial results are shown below.
        </p>
      )}
      <p className="mono meta roots-line">Scanned: {s.roots.join(", ")}</p>

      <div className="stat-grid" role="list">
        <div className="stat-card primary" role="listitem">
          <span className="stat-value">{summary.files_scanned.toLocaleString()}</span>
          <span className="stat-label">Files scanned</span>
          <span className="stat-sub">{formatBytes(summary.total_size_bytes)} total</span>
        </div>
        <button className="stat-card clickable" role="listitem" onClick={() => onNavigate("duplicates")}>
          <span className="stat-value">{summary.duplicate_groups.toLocaleString()}</span>
          <span className="stat-label">Duplicate groups</span>
          <span className="stat-sub">
            {summary.duplicate_files.toLocaleString()} files ·{" "}
            {formatBytes(summary.potential_reclaimable_bytes)} reclaimable
          </span>
        </button>
        <button className="stat-card clickable" role="listitem" onClick={() => onNavigate("empty")}>
          <span className="stat-value">{summary.empty_files.toLocaleString()}</span>
          <span className="stat-label">Empty files</span>
          <span className="stat-sub">Review before deciding anything</span>
        </button>
        <button className="stat-card clickable" role="listitem" onClick={() => onNavigate("problems")}>
          <span className="stat-value">
            {(summary.inaccessible_files + summary.error_files).toLocaleString()}
          </span>
          <span className="stat-label">Inaccessible & errors</span>
          <span className="stat-sub">
            {summary.inaccessible_files} inaccessible · {summary.error_files} with errors
          </span>
        </button>
      </div>

      <div className="card">
        <h3>What these results mean</h3>
        <ul className="explanation">
          <li>
            <strong>Duplicates</strong> are files whose full content is byte-for-byte
            identical (same size + matching SHA-256). File names play no role.
          </li>
          <li>
            <strong>Empty files</strong> may be intentional (flags, placeholders,
            lock markers) — empty does not mean useless.
          </li>
          <li>
            Nothing is deleted automatically. Cleanup moves files into{" "}
            <strong>Quarantine</strong>, from where they can be restored or
            permanently deleted after confirmation.
          </li>
        </ul>
      </div>
    </section>
  );
}
