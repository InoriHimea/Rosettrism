import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { RefreshCw, Search, Sparkles, X } from 'lucide-react';
import { LyricPlaybackView } from '../LyricPlaybackView.jsx';
import { normalizeLyricPayload } from '../lyricPlayback.js';
import { apiErrorAdvice, hasProviderWarning, normalizeApiError } from '../api/errors.js';
import { aggregateMembers, displaySource, formatDurationMs, hasSingingAnnotations, isAggregateResult, isQqResult, mergeEntryExtra } from '../utils/lyricResults.js';

const fallbackSourceOptions = ['', 'netease', 'qq', 'kugou', 'lrclib', 'migu', 'utaten', 'joysound', 'uta-net', 'lyrical-nonsense'];

export function FetchView({
  t,
  body,
  setBody,
  source,
  setSource,
  providerSources = [],
  format,
  setFormat,
  busy,
  error,
  result,
  searchResults,
  searchWarnings,
  selectedResult,
  resultDetail,
  resultDetailData,
  resultDetailBusy,
  lyricSettings,
  resultDetailScrollTick,
  searchLyric,
  fetchAggregate,
  openResultDetail,
  closeResultDetail,
  fetchSelectedResult,
  refreshMeta,
  setActiveView,
}) {
  const sourceOptions = useMemo(() => buildSourceOptions(providerSources, t), [providerSources, t]);
  const hasSearchInput = [body.query, body.title, body.artist, body.id].some((value) => String(value || '').trim());
  const canAggregate = !source && String(body.query || body.title || body.artist || '').trim();

  return (
    <section className="panel">
      <div className="panel-title">
        <h2>{t.fetch}</h2>
        <button className="button-icon" type="button" onClick={refreshMeta} title={t.refresh} aria-label={t.refresh}>
          <RefreshCw size={18} />
        </button>
      </div>
      <form className="fetch-form" onSubmit={searchLyric}>
        <label className="field-label primary-search">
          {t.keywordSearch}
          <input
            value={body.query}
            onChange={(event) => setBody({ ...body, query: event.target.value })}
            placeholder={t.query}
          />
          <span>{t.searchHint}</span>
        </label>
        <div className="search-fields">
          <label className="field-label">
            {t.titleSearch}
            <input
              value={body.title}
              onChange={(event) => setBody({ ...body, title: event.target.value })}
              placeholder={t.titleSearch}
            />
          </label>
          <label className="field-label">
            {t.artistSearch}
            <input
              value={body.artist}
              onChange={(event) => setBody({ ...body, artist: event.target.value })}
              placeholder={t.artistSearch}
            />
          </label>
          <label className="field-label">
            {t.idSearch}
            <input
              value={body.id}
              onChange={(event) => setBody({ ...body, id: event.target.value })}
              placeholder={t.idSearch}
            />
          </label>
        </div>
        <details className="advanced-options">
          <summary>{t.advancedOptions}</summary>
          <div className="controls">
            <label className="field-label">
              {t.source}
              <select value={source} onChange={(event) => setSource(event.target.value)}>
                {sourceOptions.map((option) => (
                  <option value={option.value} key={option.value || 'aggregate'} title={option.description}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="field-label">
              {t.format}
              <select value={format} onChange={(event) => setFormat(event.target.value)} disabled={!source}>
                <option value="">{t.format}</option>
                <option value="json">{t.json}</option>
                <option value="raw">{t.raw}</option>
              </select>
            </label>
            <label className="field-label">
              {t.mergeMode || t.aggregate}
              <select
                value={body.merge_mode}
                onChange={(event) => setBody({ ...body, merge_mode: event.target.value })}
                disabled={Boolean(source)}
              >
                <option value="tracks">{t.tracks}</option>
                <option value="inline">{t.inline}</option>
              </select>
            </label>
            <label className="field-label">
              {t.top}
              <input
                type="number"
                min="1"
                max={source ? '100' : '10'}
                placeholder={t.top}
                value={body.top}
                onChange={(event) =>
                  setBody({
                    ...body,
                    top: event.target.value === '' ? '' : Number(event.target.value),
                  })
                }
              />
            </label>
            <label className="field-label">
              {t.ttl}
              <input
                type="number"
                min="60"
                value={body.ttl_seconds}
                onChange={(event) => setBody({ ...body, ttl_seconds: Number(event.target.value) })}
              />
            </label>
            <label className="check">
              <input
                type="checkbox"
                checked={Boolean(body.force)}
                onChange={(event) => setBody({ ...body, force: event.target.checked })}
              />
              {t.force}
            </label>
          </div>
        </details>
        <div className="form-actions">
          <button className="button-primary" type="submit" disabled={busy || !hasSearchInput}>
            <Search size={18} />
            {busy ? t.fetching : t.searchLyrics}
          </button>
          <button className="button-secondary" type="button" disabled={busy || !canAggregate} onClick={fetchAggregate}>
            <Sparkles size={18} />
            {t.aggregateFetch}
          </button>
        </div>
      </form>
      {error ? <ErrorMessage t={t} error={error} setActiveView={setActiveView} /> : null}
      <SearchResultsList
        t={t}
        results={searchResults}
        warnings={searchWarnings}
        busy={busy}
        touched={Boolean(error || result || searchResults.length || searchWarnings.length)}
        openResultDetail={openResultDetail}
      />
      {result ? <pre className="result">{result}</pre> : null}
      <ResultDialog
        t={t}
        entry={selectedResult}
        detail={resultDetail}
        detailData={resultDetailData}
        busy={resultDetailBusy}
        lyricSettings={lyricSettings}
        scrollTick={resultDetailScrollTick}
        close={closeResultDetail}
        fetchSelectedResult={fetchSelectedResult}
      />
    </section>
  );
}

const SEARCH_RESULTS_PAGE_SIZE = 10;

function SearchResultsList({ t, results, warnings, busy, touched, openResultDetail }) {
  const [page, setPage] = useState(1);
  const pageCount = Math.max(1, Math.ceil(results.length / SEARCH_RESULTS_PAGE_SIZE));
  const safePage = Math.min(page, pageCount);
  const startIndex = (safePage - 1) * SEARCH_RESULTS_PAGE_SIZE;
  const endIndex = Math.min(results.length, startIndex + SEARCH_RESULTS_PAGE_SIZE);
  const pagedResults = results.slice(startIndex, endIndex);
  const statusText = busy
    ? t.searchingResults
    : results.length > 0
      ? `${results.length} ${t.foundResults}`
      : touched
        ? t.noResults
        : t.searchHint;

  useEffect(() => {
    setPage(1);
  }, [results, busy]);

  return (
    <div className="search-results-panel" aria-live="polite">
      <div className="search-results-title">
        <div>
          <h3>{t.searchResults}</h3>
          <span>{statusText}</span>
        </div>
        {results.length > 0 ? <span className="status-badge">{results.length} {t.candidates}</span> : null}
      </div>
      {warnings.length > 0 ? (
        <details className="warning-list">
          <summary>{t.warnings}: {warnings.length}</summary>
          {hasProviderWarning(warnings) ? <p>{t.apiError_provider_warning}</p> : null}
          <ul>
            {warnings.slice(0, 8).map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        </details>
      ) : null}
      {busy ? (
        <div className="loading-state">
          <RefreshCw size={18} />
          <span>{t.searchingResults}</span>
        </div>
      ) : results.length === 0 ? (
        <span className="empty-state">{touched ? t.noResults : t.searchHint}</span>
      ) : (
        <>
          <div className="result-table">
          {pagedResults.map((entry) => (
            <button
              type="button"
              className="result-card"
              key={`${entry.source}:${entry.id}:${entry.title}`}
              onClick={() => openResultDetail(entry)}
              title={t.openDetail}
            >
              <span className="result-source-pill">{displaySource(entry)}</span>
              <span className="result-main">
                <span className="result-title-row">
                  <strong>{entry.title || '-'}</strong>
                  {isQqResult(entry) ? (
                    <span className={`result-tag ${hasSingingAnnotations(entry) ? 'result-tag-annotation' : 'result-tag-muted'}`}>
                      {hasSingingAnnotations(entry) ? t.singingAnnotationTag : t.singingAnnotationUnavailableTag}
                    </span>
                  ) : null}
                </span>
                <span>{entry.artist || '-'}</span>
              </span>
              <span className="result-meta">
                <span>{entry.id}</span>
                <b>{formatDurationMs(entry.duration_ms)}</b>
              </span>
            </button>
          ))}
          </div>
          {pageCount > 1 ? (
          <div className="result-pagination">
            <button className="button-secondary" type="button" onClick={() => setPage((value) => Math.max(1, value - 1))} disabled={safePage <= 1}>
              {t.previousPage}
            </button>
            <span>
              {t.pageStatus.replace('{page}', safePage).replace('{total}', pageCount)}
              <small>{t.resultRange.replace('{start}', startIndex + 1).replace('{end}', endIndex).replace('{total}', results.length)}</small>
            </span>
            <button className="button-secondary" type="button" onClick={() => setPage((value) => Math.min(pageCount, value + 1))} disabled={safePage >= pageCount}>
              {t.nextPage}
            </button>
          </div>
          ) : null}
        </>
      )}
    </div>
  );
}

function ErrorMessage({ t, error, setActiveView }) {
  const normalized = normalizeApiError(error);
  const advice = apiErrorAdvice(normalized, t);
  return (
    <div className="error" role="alert">
      <strong>{t.errorTitle}</strong>
      {normalized?.code ? <span className="status-badge">{normalized.code}</span> : null}
      <pre>{normalized?.message || String(error)}</pre>
      {advice ? <p>{advice}</p> : null}
      {normalized?.details ? <pre>{JSON.stringify(normalized.details, null, 2)}</pre> : null}
      {normalized?.code === 'auth_missing_or_invalid' ? (
        <button className="button-secondary" type="button" onClick={() => setActiveView?.('settings')}>
          {t.openSettings}
        </button>
      ) : null}
    </div>
  );
}

function ResultDialog({ t, entry, detail, detailData, busy, lyricSettings, scrollTick, close, fetchSelectedResult }) {
  const dialogRef = useRef(null);
  const playbackRef = useRef(null);
  const lyricPlayback = useMemo(
    () => normalizeLyricPayload({
      ...(detailData || {}),
      selectedEntry: mergeEntryExtra(entry, detailData?.selectedEntry),
    }),
    [detailData, entry],
  );

  useEffect(() => {
    if (!entry) {
      return undefined;
    }
    function handleKeyDown(event) {
      if (event.key === 'Escape') {
        close();
      }
    }
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [entry, close]);

  useEffect(() => {
    if (!scrollTick || !lyricPlayback.playable || !playbackRef.current) {
      return;
    }
    playbackRef.current.scrollIntoView({ block: 'start', behavior: 'smooth' });
  }, [scrollTick, lyricPlayback.playable]);

  if (!entry) {
    return null;
  }
  const aggregate = isAggregateResult(entry);
  const aggregateSources = aggregateMembers(entry)
    .map((member) => displaySource(member))
    .filter((source, index, sources) => source && sources.indexOf(source) === index);

  return createPortal(
    <div className="dialog-backdrop" role="presentation" onMouseDown={close}>
      <section
        className="result-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="result-dialog-title"
        ref={dialogRef}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="dialog-title">
          <div>
            <span>{t.selectedCandidate}</span>
            <h3 id="result-dialog-title">{entry.title || '-'}</h3>
            <p>{[entry.artist, displaySource(entry), entry.id].filter(Boolean).join(' / ')}</p>
          </div>
          <button className="button-icon" type="button" onClick={close} title={t.close} aria-label={t.close}>
            <X size={18} />
          </button>
        </div>
        <div className="dialog-actions">
          <button className="button-secondary" type="button" onClick={() => fetchSelectedResult('raw')} disabled={busy || aggregate}>
            {t.fetchRaw}
          </button>
          <button className="button-primary" type="button" onClick={() => fetchSelectedResult('json', false)} disabled={busy}>
            {t.fetchJson}
          </button>
          <button className="button-primary" type="button" onClick={() => fetchSelectedResult('json', true)} disabled={busy}>
            {t.fetchEnrichedJson}
          </button>
        </div>
        <dl className="detail-grid">
          <div>
            <dt>{t.title}</dt>
            <dd>{entry.title || '-'}</dd>
          </div>
          <div>
            <dt>{t.artist}</dt>
            <dd>{entry.artist || '-'}</dd>
          </div>
          <div>
            <dt>{t.source}</dt>
            <dd>{displaySource(entry)}</dd>
          </div>
          <div>
            <dt>{t.itemId}</dt>
            <dd>{entry.id}</dd>
          </div>
          <div>
            <dt>{t.duration}</dt>
            <dd>{formatDurationMs(entry.duration_ms)}</dd>
          </div>
        </dl>
        {aggregateSources.length > 0 ? (
          <div className="detail-section">
            <strong>{t.aggregateSources}</strong>
            <pre className="cache-preview">{aggregateSources.join(' + ')}</pre>
          </div>
        ) : null}
        <div className="detail-section">
          <strong>{t.extra}</strong>
          <pre className="cache-preview">{JSON.stringify(entry.extra || {}, null, 2)}</pre>
        </div>
        {lyricPlayback.playable ? (
          <div className="dialog-playback-section" ref={playbackRef}>
            <LyricPlaybackView lyric={lyricPlayback} settings={lyricSettings} t={t} />
          </div>
        ) : detailData ? (
          <p className="hint">{t.lyricPreviewUnavailable}</p>
        ) : null}
        <details className="detail-section raw-json-section" open={!lyricPlayback.playable}>
          <summary>{busy ? t.fetching : t.rawJson}</summary>
          <pre className="cache-preview result-preview">{detail || t.resultEmpty}</pre>
        </details>
      </section>
    </div>,
    document.body,
  );
}


function buildSourceOptions(providerSources, t) {
  const aggregate = { value: '', label: t.aggregate, description: t.aggregate };
  if (!Array.isArray(providerSources) || providerSources.length === 0) {
    return [aggregate, ...fallbackSourceOptions.slice(1).map((value) => ({ value, label: value, description: value }))];
  }

  return [
    aggregate,
    ...providerSources
      .filter((source) => source?.capabilities?.search !== false)
      .map((source) => {
        const caps = capabilityBadges(source.capabilities || {}, t);
        const flags = [
          source.capabilities?.experimental ? t.experimental : null,
          source.capabilities?.requires_cookie ? t.requiresCookie : null,
          source.auth === 'required_token' ? t.requiresToken : null,
        ].filter(Boolean);
        const suffix = [...caps, ...flags].join(' · ');
        return {
          value: source.source_name,
          label: `${source.display_name || source.source_name}${suffix ? ` (${suffix})` : ''}`,
          description: `${source.source_name}: ${suffix || t.noSpecialRequirements || 'standard provider'}`,
        };
      }),
  ];
}

function capabilityBadges(capabilities, t) {
  return [
    capabilities.direct_id ? t.directId : null,
    capabilities.word_timing ? t.wordTiming : null,
    capabilities.translation ? t.translation : null,
    capabilities.romanized ? t.romanized : null,
    capabilities.ruby ? t.ruby : null,
  ].filter(Boolean);
}
