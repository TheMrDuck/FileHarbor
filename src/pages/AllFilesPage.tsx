import { useCallback, useEffect, useState } from "react";
import { api, formatBytes, formatDate } from "../api";

interface Props {
  sessionId: number;
}

const PAGE_SIZE = 100;

export default function AllFilesPage({ sessionId }: Props) {
  const [items, setItems] = useState<Awaited<ReturnType<typeof api.listFiles>>["items"]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [search, setSearch] = useState("");
  const [extension, setExtension] = useState("");
  const [sort, setSort] = useState("name");
  const [ascending, setAscending] = useState(true);
  const [loading, setLoading] = useState(true);

  const load = useCallback(
    async (p: number) => {
      setLoading(true);
      try {
        const res = await api.listFiles({
          session_id: sessionId,
          search: search || undefined,
          extension: extension || undefined,
          sort,
          ascending,
          page: p,
          page_size: PAGE_SIZE,
        });
        setItems(res.items);
        setTotal(res.total);
        setPage(p);
      } finally {
        setLoading(false);
      }
    },
    [sessionId, search, extension, sort, ascending]
  );

  useEffect(() => {
    const t = window.setTimeout(() => load(0), 250);
    return () => window.clearTimeout(t);
  }, [load]);

  return (
    <section>
      <div className="page-header">
        <h2>All Files</h2>
        <span className="meta">{total.toLocaleString()} file(s)</span>
      </div>

      <form className="filter-bar" onSubmit={(e) => e.preventDefault()}>
        <label>
          <span className="sr-only">Search by name or path</span>
          <input
            type="search"
            placeholder="Search name or path…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </label>
        <label>
          <span className="sr-only">Extension filter</span>
          <input
            type="text"
            placeholder="ext (e.g. pdf)"
            value={extension}
            onChange={(e) => setExtension(e.target.value)}
            size={10}
          />
        </label>
        <label>
          <span className="sr-only">Sort by</span>
          <select value={sort} onChange={(e) => setSort(e.target.value)}>
            <option value="name">Name</option>
            <option value="path">Path</option>
            <option value="size">Size</option>
            <option value="modified">Modified</option>
            <option value="created">Created</option>
          </select>
        </label>
        <button
          type="button"
          className="btn btn-ghost btn-small"
          onClick={() => setAscending((a) => !a)}
        >
          {ascending ? "Ascending ▲" : "Descending ▼"}
        </button>
      </form>

      {loading ? (
        <p>Loading…</p>
      ) : total === 0 ? (
        <div className="empty-state">
          <h3>No files match</h3>
          <p>Try adjusting the search or filters.</p>
        </div>
      ) : (
        <>
          <table className="file-table">
            <thead>
              <tr>
                <th scope="col">File</th>
                <th scope="col">Path</th>
                <th scope="col">Size</th>
                <th scope="col">Modified</th>
                <th scope="col">State</th>
              </tr>
            </thead>
            <tbody>
              {items.map((f) => (
                <tr key={f.id}>
                  <td className="mono">{f.file_name}</td>
                  <td className="mono truncated" title={f.path}>{f.path}</td>
                  <td>{f.is_empty ? "empty" : formatBytes(f.size_bytes)}</td>
                  <td>{formatDate(f.modified_at_ms)}</td>
                  <td>
                    {f.is_empty ? (
                      <span className="badge status-empty">Empty</span>
                    ) : f.hash_status === "computed" ? (
                      <span className="badge status-completed">Analyzed</span>
                    ) : (
                      <span className="badge">{f.scan_status.replace(/_/g, " ")}</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <nav className="pager" aria-label="File pages">
            <button className="btn btn-secondary" disabled={page === 0} onClick={() => load(page - 1)}>
              Previous
            </button>
            <span className="meta">
              Page {page + 1} · showing {items.length} of {total.toLocaleString()}
            </span>
            <button
              className="btn btn-secondary"
              disabled={(page + 1) * PAGE_SIZE >= total}
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
