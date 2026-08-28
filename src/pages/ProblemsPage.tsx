import { useCallback, useEffect, useState } from "react";
import { api, formatBytes, formatDate } from "../api";
import type { FileRecord } from "../types";

interface Props {
  sessionId: number;
}

export default function ProblemsPage({ sessionId }: Props) {
  const [items, setItems] = useState<FileRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [retrying, setRetrying] = useState(false);

  const load = useCallback(
    async (p: number) => {
      const res = await api.problemFiles(sessionId, p);
      setItems(res.items);
      setTotal(res.total);
      setPage(p);
    },
    [sessionId]
  );

  useEffect(() => {
    load(0);
  }, [load]);

  const retryAnalysis = async () => {
    setRetrying(true);
    try {
      await api.rebuildAnalysis(sessionId);
      await load(page);
    } finally {
      setRetrying(false);
    }
  };

  return (
    <section>
      <div className="page-header">
        <h2>Inaccessible & Errors</h2>
        <div className="row">
          <span className="meta">{total.toLocaleString()} item(s)</span>
          <button className="btn btn-secondary" onClick={retryAnalysis} disabled={retrying}>
            {retrying ? "Retrying…" : "Retry analysis"}
          </button>
        </div>
      </div>

      {total === 0 ? (
        <div className="empty-state">
          <h3>No problems recorded</h3>
          <p>Every scanned file was readable when the scan ran.</p>
        </div>
      ) : (
        <>
          <table className="file-table">
            <thead>
              <tr>
                <th scope="col">State</th>
                <th scope="col">Path</th>
                <th scope="col">Size</th>
                <th scope="col">Modified</th>
                <th scope="col">What happened</th>
              </tr>
            </thead>
            <tbody>
              {items.map((f) => (
                <tr key={f.id}>
                  <td>
                    <span className={`badge status-${f.scan_status}`}>
                      {humanStatus(f.scan_status)}
                    </span>
                  </td>
                  <td className="mono truncated" title={f.path}>{f.path}</td>
                  <td>{f.scan_status === "inaccessible" ? "-" : formatBytes(f.size_bytes)}</td>
                  <td>{formatDate(f.modified_at_ms)}</td>
                  <td className="error-cell">{f.error ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <nav className="pager" aria-label="Problem pages">
            <button className="btn btn-secondary" disabled={page === 0} onClick={() => load(page - 1)}>
              Previous
            </button>
            <button
              className="btn btn-secondary"
              disabled={(page + 1) * 100 >= total}
              onClick={() => load(page + 1)}
            >
              Next
            </button>
          </nav>
        </>
      )}
    </section>
  );
}

function humanStatus(status: string): string {
  switch (status) {
    case "inaccessible":
      return "Inaccessible";
    case "hash_failed":
      return "Hash failed";
    case "changed_during_scan":
      return "Changed during scan";
    case "skipped":
      return "Skipped";
    default:
      return status;
  }
}
