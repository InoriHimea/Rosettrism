import React from 'react';
import { RefreshCw, Trash2 } from 'lucide-react';
import { RecentRunsPanel } from './OverviewView.jsx';
import { displaySource } from '../utils/lyricResults.js';
import { formatTimestamp } from '../utils/format.js';
import { formatApiErrorForDisplay } from '../api/errors.js';

const upstreamType = 'upstream';
const unifiedType = 'unified';

export function CacheView({
  t,
  cache,
  unifiedCache = [],
  stats,
  selectedCacheEntry,
  cacheDetail,
  cacheDetailBusy,
  selectCacheEntry,
  deleteCache,
  refreshMeta,
}) {
  const activeType = selectedCacheEntry?.cache_type || upstreamType;
  const upstreamDetail = cacheDetail?.entry || (activeType === upstreamType ? selectedCacheEntry : null);
  const unifiedDetail = cacheDetail?.unified_entry || (activeType === unifiedType ? selectedCacheEntry : null);
  const detailEntry = unifiedDetail || upstreamDetail;

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
          <CacheSectionTitle title={t.upstreamCache} count={cache.length} />
          <div className="cache-row cache-row-head">
            <strong>{t.item}</strong>
            <span>{t.operation}</span>
            <span>{t.status}</span>
            <span>{t.size}</span>
            <span />
          </div>
          {cache.length === 0 ? <span className="empty-state">{t.noData}</span> : null}
          {cache.map((entry) => (
            <CacheRow
              key={`upstream-${entry.id}`}
              t={t}
              entry={entry}
              selected={isSelected(selectedCacheEntry, entry)}
              selectCacheEntry={selectCacheEntry}
              deleteCache={deleteCache}
            />
          ))}

          <CacheSectionTitle title={t.unified} count={unifiedCache.length} />
          <div className="cache-row cache-row-head unified-cache-row-head">
            <strong>{t.item}</strong>
            <span>{t.dependencies}</span>
            <span>{t.status}</span>
            <span>{t.hash}</span>
            <span />
          </div>
          {unifiedCache.length === 0 ? <span className="empty-state">{t.noData}</span> : null}
          {unifiedCache.map((entry) => (
            <CacheRow
              key={`unified-${entry.id}`}
              t={t}
              entry={entry}
              selected={isSelected(selectedCacheEntry, entry)}
              selectCacheEntry={selectCacheEntry}
              deleteCache={deleteCache}
            />
          ))}
        </div>

        <aside className="cache-detail" aria-live="polite">
          {cacheDetailBusy ? (
            <span className="empty-state">{t.checking}</span>
          ) : cacheDetail?.error ? (
            <pre className="error">{formatApiErrorForDisplay(cacheDetail.error, t)}</pre>
          ) : detailEntry ? (
            <CacheDetail
              t={t}
              cacheDetail={cacheDetail}
              upstreamDetail={upstreamDetail}
              unifiedDetail={unifiedDetail}
              detailEntry={detailEntry}
            />
          ) : (
            <span className="empty-state">{t.selectEntry}</span>
          )}
        </aside>
      </div>
    </section>
  );
}

function CacheSectionTitle({ title, count }) {
  return (
    <div className="cache-section-title">
      <strong>{title}</strong>
      <span>{count}</span>
    </div>
  );
}

function CacheRow({ t, entry, selected, selectCacheEntry, deleteCache }) {
  const primary = cacheEntryPrimary(entry);
  const secondary = cacheEntrySecondary(entry, t);
  const isUnified = entry.cache_type === unifiedType;
  return (
    <div
      className={`cache-row ${selected ? 'cache-row-active' : ''}`}
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
      <span data-label={isUnified ? t.dependencies : t.operation}>
        {isUnified ? entry.dependency_count ?? 0 : entry.operation}
      </span>
      <span data-label={t.status} className={`status-badge ${entry.fresh ? 'status-fresh' : 'status-expired'}`}>
        {entry.fresh ? t.fresh : t.expired}
      </span>
      <span data-label={isUnified ? t.hash : t.size}>{isUnified ? shortHash(entry.body_hash) : formatBytes(entry.body_len)}</span>
      <button
        className="button-danger button-icon"
        type="button"
        onClick={(event) => {
          event.stopPropagation();
          deleteCache(entry);
        }}
        title={t.delete}
        aria-label={`${t.delete} ${primary}`}
      >
        <Trash2 size={16} />
      </button>
    </div>
  );
}

function CacheDetail({ t, cacheDetail, upstreamDetail, unifiedDetail, detailEntry }) {
  return (
    <>
      <div className="cache-detail-head">
        <span>{unifiedDetail ? t.unified : t.detail}</span>
        <strong>{cacheEntryPrimary(detailEntry)}</strong>
        <small>{cacheEntrySecondary(detailEntry, t)}</small>
      </div>
      <dl className="detail-grid">
        {upstreamDetail ? (
          <>
            <div>
              <dt>{t.source}</dt>
              <dd>{upstreamDetail.source}</dd>
            </div>
            <div>
              <dt>{t.operation}</dt>
              <dd>{upstreamDetail.operation}</dd>
            </div>
            <div>
              <dt>{t.statusCode}</dt>
              <dd>{upstreamDetail.status_code}</dd>
            </div>
          </>
        ) : null}
        {unifiedDetail ? (
          <div>
            <dt>{t.dependencies}</dt>
            <dd>{dependencyCount(unifiedDetail.dependencies, unifiedDetail.dependency_count)}</dd>
          </div>
        ) : null}
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
      {upstreamDetail ? (
        <>
          <div className="detail-section">
            <strong>{t.metadata}</strong>
            <pre className="cache-preview">{JSON.stringify(upstreamDetail.metadata || {}, null, 2)}</pre>
          </div>
          <div className="detail-section">
            <strong>{t.preview}</strong>
            <pre className="cache-preview">{upstreamDetail.body_text_preview || t.bodyPreviewEmpty}</pre>
          </div>
        </>
      ) : null}
      {unifiedDetail ? (
        <>
          <div className="detail-section">
            <strong>{t.preview}</strong>
            <pre className="cache-preview">{formatJsonPreview(unifiedDetail.body_text_preview, t.bodyPreviewEmpty)}</pre>
          </div>
          <div className="detail-section">
            <strong>{t.dependencies}</strong>
            <pre className="cache-preview">{JSON.stringify(unifiedDetail.dependencies || [], null, 2)}</pre>
          </div>
          <div className="detail-section">
            <strong>{t.aiScoreDetails}</strong>
            <pre className="cache-preview">{JSON.stringify(cacheDetail?.ai_scores || [], null, 2)}</pre>
          </div>
        </>
      ) : null}
    </>
  );
}

function isSelected(selected, entry) {
  return selected?.id === entry.id && (selected.cache_type || upstreamType) === (entry.cache_type || upstreamType);
}

function cacheEntryPrimary(entry) {
  if (!entry) {
    return '-';
  }
  if (entry.cache_type === unifiedType || entry.dependencies || entry.dependency_count !== undefined) {
    return `Unified #${entry.id}`;
  }
  return entry.title || entry.query || entry.item_id || `${entry.source || 'cache'} #${entry.id}`;
}

function cacheEntrySecondary(entry, t) {
  if (!entry) {
    return '';
  }
  if (entry.cache_type === unifiedType || entry.dependencies || entry.dependency_count !== undefined) {
    return [
      `${t.dependencies}: ${dependencyCount(entry.dependencies, entry.dependency_count)}`,
      entry.cache_key,
    ]
      .filter(Boolean)
      .join(' / ');
  }
  return [
    entry.artist,
    entry.item_id ? `${t.itemId}: ${entry.item_id}` : null,
    entry.query && entry.query !== entry.title ? `${t.queryLabel}: ${entry.query}` : null,
    displaySource(entry.source),
  ]
    .filter(Boolean)
    .join(' / ');
}

function dependencyCount(dependencies, fallback = 0) {
  return Array.isArray(dependencies) ? dependencies.length : fallback ?? 0;
}

function shortHash(hash) {
  return hash ? `${hash.slice(0, 10)}…` : '-';
}

function formatJsonPreview(preview, emptyText) {
  if (!preview) {
    return emptyText;
  }
  try {
    return JSON.stringify(JSON.parse(preview), null, 2);
  } catch {
    return preview;
  }
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
