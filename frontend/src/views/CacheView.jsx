import { RefreshCw, Trash2 } from 'lucide-react';
import { RecentRunsPanel } from './OverviewView.jsx';
import { displaySource } from '../utils/lyricResults.js';
import { formatTimestamp } from '../utils/format.js';

export function CacheView({
  t,
  cache,
  stats,
  selectedCacheEntry,
  cacheDetail,
  cacheDetailBusy,
  selectCacheEntry,
  deleteCache,
  refreshMeta,
}) {
  const detailEntry = cacheDetail && !cacheDetail.error ? cacheDetail : selectedCacheEntry;

  return (
    <section className="panel">
      <div className="panel-title">
        <h2>{t.cache}</h2>
        <button className="button-icon" type="button" onClick={refreshMeta} title={t.refresh} aria-label={t.refresh}>
          <RefreshCw size={18} />
        </button>
      </div>
      <RecentRunsPanel t={t} runs={stats?.fetch_runs || []} />
      <div className="cache-layout">
        <div className="cache-list">
          <div className="cache-row cache-row-head">
            <strong>{t.item}</strong>
            <span>{t.operation}</span>
            <span>{t.status}</span>
            <span>{t.size}</span>
            <span />
          </div>
          {cache.length === 0 ? <span className="empty-state">{t.noData}</span> : null}
          {cache.map((entry) => {
            const primary = cacheEntryPrimary(entry);
            const secondary = cacheEntrySecondary(entry, t);
            const selected = selectedCacheEntry?.id === entry.id;
            return (
              <div
                className={`cache-row ${selected ? 'cache-row-active' : ''}`}
                key={entry.id}
                role="button"
                tabIndex={0}
                onClick={() => selectCacheEntry(entry)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' || event.key === ' ') {
                    event.preventDefault();
                    selectCacheEntry(entry);
                  }
                }}
              >
                <div className="cache-main">
                  <strong>{primary}</strong>
                  <span>{secondary}</span>
                </div>
                <span data-label={t.operation}>{entry.operation}</span>
                <span data-label={t.status} className={`status-badge ${entry.fresh ? 'status-fresh' : 'status-expired'}`}>
                  {entry.fresh ? t.fresh : t.expired}
                </span>
                <span data-label={t.size}>{formatBytes(entry.body_len)}</span>
                <button
                  className="button-danger button-icon"
                  type="button"
                  onClick={(event) => {
                    event.stopPropagation();
                    deleteCache(entry.id);
                  }}
                  title={t.delete}
                  aria-label={`${t.delete} ${primary}`}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            );
          })}
        </div>

        <aside className="cache-detail" aria-live="polite">
          {cacheDetailBusy ? (
            <span className="empty-state">{t.checking}</span>
          ) : cacheDetail?.error ? (
            <pre className="error">{cacheDetail.error}</pre>
          ) : detailEntry ? (
            <>
              <div className="cache-detail-head">
                <span>{t.detail}</span>
                <strong>{cacheEntryPrimary(detailEntry)}</strong>
                <small>{cacheEntrySecondary(detailEntry, t)}</small>
              </div>
              <dl className="detail-grid">
                <div>
                  <dt>{t.source}</dt>
                  <dd>{detailEntry.source}</dd>
                </div>
                <div>
                  <dt>{t.operation}</dt>
                  <dd>{detailEntry.operation}</dd>
                </div>
                <div>
                  <dt>{t.statusCode}</dt>
                  <dd>{detailEntry.status_code}</dd>
                </div>
                <div>
                  <dt>{t.hash}</dt>
                  <dd>{detailEntry.body_hash}</dd>
                </div>
                <div>
                  <dt>{t.createdAt}</dt>
                  <dd>{formatTimestamp(detailEntry.created_at)}</dd>
                </div>
                <div>
                  <dt>{t.expiresAt}</dt>
                  <dd>{formatTimestamp(detailEntry.expires_at)}</dd>
                </div>
                <div>
                  <dt>{t.cacheKey}</dt>
                  <dd>{detailEntry.cache_key}</dd>
                </div>
              </dl>
              {cacheDetail ? (
                <>
                  <div className="detail-section">
                    <strong>{t.metadata}</strong>
                    <pre className="cache-preview">{JSON.stringify(cacheDetail.metadata || {}, null, 2)}</pre>
                  </div>
                  <div className="detail-section">
                    <strong>{t.preview}</strong>
                    <pre className="cache-preview">
                      {cacheDetail.body_text_preview || t.bodyPreviewEmpty}
                    </pre>
                  </div>
                </>
              ) : null}
            </>
          ) : (
            <span className="empty-state">{t.selectEntry}</span>
          )}
        </aside>
      </div>
    </section>
  );
}

function cacheEntryPrimary(entry) {
  if (!entry) {
    return '-';
  }
  return entry.title || entry.query || entry.item_id || `${entry.source || 'cache'} #${entry.id}`;
}

function cacheEntrySecondary(entry, t) {
  if (!entry) {
    return '';
  }
  return [
    entry.artist,
    entry.item_id ? `${t.itemId}: ${entry.item_id}` : null,
    entry.query && entry.query !== entry.title ? `${t.queryLabel}: ${entry.query}` : null,
    entry.source,
  ]
    .filter(Boolean)
    .join(' / ');
}

function formatBytes(bytes) {
  if (!Number.isFinite(bytes)) {
    return '-';
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
