import { useCallback, useEffect, useState } from "react";
import { api, formatDate } from "../api";
import type { FileOperation, OperationItem } from "../types";

export default function OperationsPage() {
  const [ops, setOps] = useState<FileOperation[]>([]);
  const [openOpId, setOpenOpId] = useState<number | null>(null);
  const [opItems, setOpItems] = useState<OperationItem[]>([]);

  const load = useCallback(async () => {
    setOps(await api.operations(0));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const toggle = async (opId: number) => {
    if (openOpId === opId) {
      setOpenOpId(null);
      return;
    }
    setOpItems(await api.operationItems(opId));
    setOpenOpId(opId);
  };

  return (
    <section>
      <div className="page-header">
        <h2>Operation History</h2>
        <button className="btn btn-ghost btn-small" onClick={load}>
          Refresh
        </button>
      </div>

      {ops.length === 0 ? (
        <div className="empty-state">
          <h3>No operations yet</h3>
          <p>
            Quarantine, restore and permanent-delete actions are recorded here so
            every change stays traceable.
          </p>
        </div>
      ) : (
        <table className="file-table">
          <thead>
            <tr>
              <th scope="col"></th>
              <th scope="col">Operation</th>
              <th scope="col">Type</th>
              <th scope="col">Status</th>
              <th scope="col">Started</th>
            </tr>
          </thead>
          <tbody>
            {ops.map((op) => (
              <>
                <tr key={op.id}>
                  <td>
                    <button
                      className="btn btn-ghost btn-small"
                      aria-expanded={openOpId === op.id}
                      onClick={() => toggle(op.id)}
                    >
                      {openOpId === op.id ? "−" : "+"}
                    </button>
                  </td>
                  <td className="mono">{op.batch_id}</td>
                  <td>{humanKind(op.kind)}</td>
                  <td>
                    <span className={`badge status-${op.status}`}>{humanStatus(op.status)}</span>
                  </td>
                  <td>{formatDate(op.started_at_ms)}</td>
                </tr>
                {openOpId === op.id && (
                  <tr key={`${op.id}-detail`}>
                    <td colSpan={5} className="op-detail">
                      {opItems.length === 0 ? (
                        <span className="meta">No item details.</span>
                      ) : (
                        <ul className="op-items">
                          {opItems.map((it) => (
                            <li key={it.id}>
                              <span className={`badge status-${it.status}`}>{it.status}</span>{" "}
                              <span className="mono truncated">{it.source ?? "?"}</span>
                              {it.error && <em className="error-cell">: {it.error}</em>}
                            </li>
                          ))}
                        </ul>
                      )}
                    </td>
                  </tr>
                )}
              </>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

function humanKind(kind: string): string {
  switch (kind) {
    case "quarantine":
      return "Move to quarantine";
    case "restore":
      return "Restore";
    case "delete_permanent":
      return "Permanent deletion";
    default:
      return kind;
  }
}

function humanStatus(status: string): string {
  return status.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}
