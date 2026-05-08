import React, { useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import {
  BarChart3,
  Database,
  Languages,
  RefreshCw,
  Search,
  Server,
  Settings,
  Sparkles,
  Trash2,
  X,
} from 'lucide-react';
import './styles.css';

const dictionaries = {
  zh: {
    item: '\u6761\u76ee',
    itemId: 'ID',
    title: '\u66f2\u540d',
    artist: '\u6b4c\u624b',
    detail: '\u8be6\u60c5',
    metadata: '\u5143\u6570\u636e',
    preview: '\u5185\u5bb9\u9884\u89c8',
    hash: 'Hash',
    createdAt: '\u521b\u5efa',
    expiresAt: '\u8fc7\u671f',
    cacheKey: '\u7f13\u5b58\u952e',
    statusCode: 'HTTP',
    queryLabel: '\u67e5\u8be2',
    selectEntry: '\u9009\u62e9\u4e00\u6761\u7f13\u5b58\u67e5\u770b\u5185\u5bb9',
    bodyPreviewEmpty: '\u54cd\u5e94\u5185\u5bb9\u4e0d\u53ef\u6309\u6587\u672c\u9884\u89c8',
    keywordSearch: '\u5173\u952e\u8bcd / URL / \u5e73\u53f0 ID',
    titleSearch: '\u6b4c\u540d',
    artistSearch: '\u827a\u672f\u5bb6',
    idSearch: '\u5217\u8868 ID \u6216\u76f4\u94fe ID',
    searchLyrics: '\u641c\u7d22',
    aggregateFetch: '\u805a\u5408\u751f\u6210',
    searchResults: '\u641c\u7d22\u7ed3\u679c',
    candidates: '\u5019\u9009',
    duration: '\u65f6\u957f',
    openDetail: '\u67e5\u770b\u8be6\u60c5',
    selectedCandidate: '\u5019\u9009\u8be6\u60c5',
    aggregateSources: '\u5408\u5e76\u6765\u6e90',
    fetchRaw: '\u83b7\u53d6\u539f\u6587',
    fetchJson: '\u83b7\u53d6 JSON',
    warnings: '\u63d0\u793a',
    noResults: '\u6682\u65e0\u641c\u7d22\u7ed3\u679c',
    extra: 'Extra',
    dashboard: '4.0 控制台',
    overview: '总览',
    fetch: '获取',
    cache: '缓存',
    inspector: '质量',
    settings: '设置',
    server: '服务',
    online: '在线',
    checking: '检查中',
    version: '版本',
    upstreamCache: '上游缓存',
    fresh: '新鲜缓存',
    expired: '过期缓存',
    unified: '统一结果',
    query: '歌曲名、歌手、URL 或平台 ID',
    aggregate: '聚合',
    source: '来源',
    format: '格式',
    raw: '原文',
    json: 'JSON',
    tracks: '多轨',
    inline: '行内',
    top: 'Top',
    ttl: 'TTL 秒',
    force: '强刷',
    runFetch: '获取',
    fetching: '获取中',
    refresh: '刷新',
    resultEmpty: '结果会显示在这里。',
    entries: '条记录',
    operation: '操作',
    status: '状态',
    size: '大小',
    delete: '删除',
    cacheHealth: '缓存健康',
    sourceMix: '来源分布',
    operationMix: '操作类型',
    recentActivity: '最近缓存',
    noData: '暂无数据',
    freshRatio: '新鲜率',
    totalEntries: '总记录',
    searchOps: '搜索',
    fetchOps: '获取',
    otherOps: '其他',
    sourcePolicy: '指定来源必须选择 raw 或 json。',
    qualityPending: 'AI 评分入口已预留；配置模型后可接入重新评分。',
    cachePath: '缓存由 --db 或 ROSETTRISM_DB 控制。',
    language: '语言',
    chinese: '中文',
    english: 'English',
  },
  en: {
    item: 'Item',
    itemId: 'ID',
    title: 'Title',
    artist: 'Artist',
    detail: 'Detail',
    metadata: 'Metadata',
    preview: 'Content preview',
    hash: 'Hash',
    createdAt: 'Created',
    expiresAt: 'Expires',
    cacheKey: 'Cache key',
    statusCode: 'HTTP',
    queryLabel: 'Query',
    selectEntry: 'Select a cache entry to inspect its content.',
    bodyPreviewEmpty: 'Response body is not previewable as text.',
    keywordSearch: 'Keyword, URL, or platform id',
    titleSearch: 'Title',
    artistSearch: 'Artist',
    idSearch: 'List id or direct id',
    searchLyrics: 'Search',
    aggregateFetch: 'Aggregate',
    searchResults: 'Search results',
    candidates: 'candidates',
    duration: 'Duration',
    openDetail: 'Open detail',
    selectedCandidate: 'Candidate detail',
    aggregateSources: 'Merged sources',
    fetchRaw: 'Fetch raw',
    fetchJson: 'Fetch JSON',
    warnings: 'Warnings',
    noResults: 'No search results',
    extra: 'Extra',
    dashboard: '4.0 dashboard',
    overview: 'Overview',
    fetch: 'Fetch',
    cache: 'Cache',
    inspector: 'Quality',
    settings: 'Settings',
    server: 'Server',
    online: 'online',
    checking: 'checking',
    version: 'Version',
    upstreamCache: 'Upstream cache',
    fresh: 'Fresh',
    expired: 'Expired',
    unified: 'Unified',
    query: 'Song title, artist, URL, or platform id',
    aggregate: 'Aggregate',
    source: 'Source',
    format: 'Format',
    raw: 'Raw',
    json: 'JSON',
    tracks: 'Tracks',
    inline: 'Inline',
    top: 'Top',
    ttl: 'TTL seconds',
    force: 'Force',
    runFetch: 'Fetch',
    fetching: 'Fetching',
    refresh: 'Refresh',
    resultEmpty: 'Result will appear here.',
    entries: 'entries',
    operation: 'Operation',
    status: 'Status',
    size: 'Size',
    delete: 'Delete',
    cacheHealth: 'Cache health',
    sourceMix: 'Source mix',
    operationMix: 'Operation mix',
    recentActivity: 'Recent cache',
    noData: 'No data',
    freshRatio: 'Fresh ratio',
    totalEntries: 'Total entries',
    searchOps: 'Search',
    fetchOps: 'Fetch',
    otherOps: 'Other',
    sourcePolicy: 'Source-specific requests require raw or json.',
    qualityPending: 'AI scoring is ready to be wired once a model is configured.',
    cachePath: 'Cache path is controlled by --db or ROSETTRISM_DB.',
    language: 'Language',
    chinese: '中文',
    english: 'English',
  },
};

const navItems = [
  ['overview', BarChart3],
  ['fetch', Search],
  ['cache', Database],
  ['inspector', Sparkles],
  ['settings', Settings],
];

const sourceOptions = ['', 'netease', 'qq', 'kugou', 'lrclib', 'migu', 'utaten', 'joysound', 'uta-net', 'lyrical-nonsense'];

const defaultBody = {
  query: '',
  title: '',
  artist: '',
  id: '',
  merge_mode: 'tracks',
  top: '',
  translation_lang: 'zh-Hans',
  ttl_seconds: 604800,
};

function App() {
  const [language, setLanguage] = useState(localStorage.getItem('rosettrism-language') || 'zh');
  const [activeView, setActiveView] = useState('overview');
  const [health, setHealth] = useState(null);
  const [stats, setStats] = useState(null);
  const [cache, setCache] = useState([]);
  const [body, setBody] = useState(defaultBody);
  const [source, setSource] = useState('');
  const [format, setFormat] = useState('');
  const [result, setResult] = useState('');
  const [searchResults, setSearchResults] = useState([]);
  const [searchWarnings, setSearchWarnings] = useState([]);
  const [selectedResult, setSelectedResult] = useState(null);
  const [resultDetail, setResultDetail] = useState('');
  const [resultDetailBusy, setResultDetailBusy] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [selectedCacheEntry, setSelectedCacheEntry] = useState(null);
  const [cacheDetail, setCacheDetail] = useState(null);
  const [cacheDetailBusy, setCacheDetailBusy] = useState(false);
  const t = dictionaries[language] || dictionaries.zh;

  useEffect(() => {
    refreshMeta();
  }, []);

  useEffect(() => {
    localStorage.setItem('rosettrism-language', language);
  }, [language]);

  const aggregatePayload = useMemo(() => {
    const aggregateQuery = [body.title, body.artist]
      .map((value) => String(value || '').trim())
      .filter(Boolean)
      .join(' ');
    const next = { ...body };
    next.query = aggregateQuery || body.query;
    delete next.title;
    delete next.artist;
    delete next.id;
    if (next.top === '' || next.top === null || next.top === undefined) {
      delete next.top;
    }
    delete next.source;
    delete next.format;
    return next;
  }, [body]);

  const searchPayload = useMemo(() => {
    const next = {
      query: body.query || undefined,
      title: body.title || undefined,
      artist: body.artist || undefined,
      id: body.id || undefined,
      source: source || undefined,
      top: body.top === '' ? undefined : body.top,
      merge_mode: source ? undefined : body.merge_mode,
      ttl_seconds: body.ttl_seconds,
      force: body.force || undefined,
    };
    return Object.fromEntries(Object.entries(next).filter(([, value]) => value !== undefined && value !== ''));
  }, [body, source]);

  async function refreshMeta() {
    const [healthRes, statsRes, cacheRes] = await Promise.all([
      fetch('/api/health'),
      fetch('/api/stats'),
      fetch('/api/cache'),
    ]);
    setHealth(await healthRes.json());
    setStats(await statsRes.json());
    const entries = (await cacheRes.json()).entries || [];
    setCache(entries);
    setSelectedCacheEntry((selected) => {
      if (!selected) {
        return selected;
      }
      return entries.find((entry) => entry.id === selected.id) || selected;
    });
  }

  async function searchLyric(event) {
    event.preventDefault();
    setBusy(true);
    setError('');
    setResult('');
    setSearchResults([]);
    setSearchWarnings([]);
    try {
      const response = await fetch('/api/search', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(searchPayload),
      });
      const text = await response.text();
      if (!response.ok) {
        throw new Error(text);
      }
      const data = JSON.parse(text);
      setSearchResults(data.results || []);
      setSearchWarnings(data.warnings || []);
      setActiveView('fetch');
      await refreshMeta();
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  async function fetchAggregate() {
    setBusy(true);
    setError('');
    setResult('');
    try {
      const response = await fetch('/api/fetch', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(aggregatePayload),
      });
      const text = await response.text();
      if (!response.ok) {
        throw new Error(text);
      }
      setResult(JSON.stringify(JSON.parse(text), null, 2));
      setActiveView('fetch');
      await refreshMeta();
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  }

  function openResultDetail(entry) {
    setSelectedResult(entry);
    setResultDetail('');
  }

  function closeResultDetail() {
    setSelectedResult(null);
    setResultDetail('');
    setResultDetailBusy(false);
  }

  async function fetchSelectedResult(requestedFormat) {
    if (!selectedResult) {
      return;
    }
    setResultDetailBusy(true);
    setError('');
    try {
      const response = await fetch('/api/fetch-result', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          result: selectedResult,
          format: requestedFormat,
          ttl_seconds: body.ttl_seconds,
          force: Boolean(body.force),
        }),
      });
      const text = await response.text();
      if (!response.ok) {
        throw new Error(text);
      }
      setResultDetail(JSON.stringify(JSON.parse(text), null, 2));
      await refreshMeta();
    } catch (err) {
      setResultDetail(err.message);
    } finally {
      setResultDetailBusy(false);
    }
  }

  async function deleteCache(id) {
    await fetch(`/api/cache/${id}`, { method: 'DELETE' });
    if (selectedCacheEntry?.id === id) {
      setSelectedCacheEntry(null);
      setCacheDetail(null);
    }
    await refreshMeta();
  }

  async function selectCacheEntry(entry) {
    setSelectedCacheEntry(entry);
    setCacheDetail(null);
    setCacheDetailBusy(true);
    try {
      const response = await fetch(`/api/cache/${entry.id}`);
      if (!response.ok) {
        throw new Error(await response.text());
      }
      const data = await response.json();
      setCacheDetail(data.entry || null);
    } catch (err) {
      setCacheDetail({ error: err.message });
    } finally {
      setCacheDetailBusy(false);
    }
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <Sparkles size={24} />
          <div>
            <h1>Rosettrism</h1>
            <p>{t.dashboard}</p>
          </div>
        </div>
        <nav aria-label="Dashboard">
          {navItems.map(([id, Icon]) => (
            <button
              type="button"
              className={activeView === id ? 'nav-active' : ''}
              onClick={() => setActiveView(id)}
              key={id}
            >
              <Icon size={18} />
              {t[id]}
            </button>
          ))}
        </nav>
        <button className="language-button" type="button" onClick={() => setLanguage(language === 'zh' ? 'en' : 'zh')}>
          <Languages size={18} />
          {language === 'zh' ? t.english : t.chinese}
        </button>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <Metric label={t.server} value={health?.ok ? t.online : t.checking} />
          <Metric label={t.version} value={health?.version || '-'} />
          <Metric label={t.upstreamCache} value={stats?.cache?.upstream_entries ?? '-'} />
          <Metric label={t.fresh} value={stats?.cache?.fresh_upstream_entries ?? '-'} />
        </header>

        {activeView === 'overview' && <Overview t={t} stats={stats} cache={cache} setActiveView={setActiveView} />}
        {activeView === 'fetch' && (
          <FetchView
            t={t}
            body={body}
            setBody={setBody}
            source={source}
            setSource={setSource}
            format={format}
            setFormat={setFormat}
            busy={busy}
            error={error}
            result={result}
            searchResults={searchResults}
            searchWarnings={searchWarnings}
            selectedResult={selectedResult}
            resultDetail={resultDetail}
            resultDetailBusy={resultDetailBusy}
            searchLyric={searchLyric}
            fetchAggregate={fetchAggregate}
            openResultDetail={openResultDetail}
            closeResultDetail={closeResultDetail}
            fetchSelectedResult={fetchSelectedResult}
            refreshMeta={refreshMeta}
          />
        )}
        {activeView === 'cache' && (
          <CacheView
            t={t}
            cache={cache}
            selectedCacheEntry={selectedCacheEntry}
            cacheDetail={cacheDetail}
            cacheDetailBusy={cacheDetailBusy}
            selectCacheEntry={selectCacheEntry}
            deleteCache={deleteCache}
            refreshMeta={refreshMeta}
          />
        )}
        {activeView === 'inspector' && <InspectorView t={t} result={result} />}
        {activeView === 'settings' && (
          <SettingsView t={t} language={language} setLanguage={setLanguage} payload={searchPayload} />
        )}
      </section>
    </main>
  );
}

function Overview({ t, stats, cache, setActiveView }) {
  const total = stats?.cache?.upstream_entries ?? cache.length;
  const fresh = stats?.cache?.fresh_upstream_entries ?? cache.filter((entry) => entry.fresh).length;
  const expired = Math.max((stats?.cache?.expired_upstream_entries ?? total - fresh) || 0, 0);
  const freshPercent = total > 0 ? Math.round((fresh / total) * 100) : 0;
  const sourceRows = groupedRows(cache, 'source').slice(0, 7);
  const operationRows = operationSummary(cache, t);
  const recentRows = cache.slice(0, 8);

  return (
    <section className="dashboard-grid">
      <article className="chart-panel health-panel">
        <div className="chart-title">
          <div>
            <h2>{t.cacheHealth}</h2>
            <p>{t.totalEntries}: {total}</p>
          </div>
          <Database size={20} />
        </div>
        <div className="donut-row">
          <div
            className="donut"
            style={{ '--fresh': `${freshPercent * 3.6}deg` }}
            aria-label={`${t.freshRatio} ${freshPercent}%`}
          >
            <strong>{freshPercent}%</strong>
            <span>{t.freshRatio}</span>
          </div>
          <div className="legend">
            <LegendDot label={t.fresh} value={fresh} color="green" />
            <LegendDot label={t.expired} value={expired} color="amber" />
            <button className="text-action" type="button" onClick={() => setActiveView('cache')}>
              {t.cache}
            </button>
          </div>
        </div>
      </article>

      <article className="chart-panel">
        <div className="chart-title">
          <div>
            <h2>{t.sourceMix}</h2>
            <p>{t.upstreamCache}</p>
          </div>
          <BarChart3 size={20} />
        </div>
        <BarList rows={sourceRows} empty={t.noData} />
      </article>

      <article className="chart-panel">
        <div className="chart-title">
          <div>
            <h2>{t.operationMix}</h2>
            <p>{t.searchOps} / {t.fetchOps}</p>
          </div>
          <Server size={20} />
        </div>
        <BarList rows={operationRows} empty={t.noData} />
      </article>

      <article className="chart-panel wide-panel">
        <div className="chart-title">
          <div>
            <h2>{t.recentActivity}</h2>
            <p>{recentRows.length} {t.entries}</p>
          </div>
          <RefreshCw size={20} />
        </div>
        <div className="spark-grid">
          {recentRows.length === 0 ? (
            <span className="empty-state">{t.noData}</span>
          ) : (
            recentRows.map((entry) => (
              <div className="spark-column" key={entry.id}>
                <span
                  className={entry.fresh ? 'spark-bar fresh-bar' : 'spark-bar expired-bar'}
                  style={{ height: `${Math.max(18, Math.min(92, Math.round((entry.body_len || 1) / 80)))}px` }}
                />
                <small>{entry.source}</small>
              </div>
            ))
          )}
        </div>
      </article>
    </section>
  );
}

function LegendDot({ label, value, color }) {
  return (
    <div className="legend-row">
      <span className={`legend-dot ${color}`} />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function BarList({ rows, empty }) {
  const max = Math.max(...rows.map((row) => row.value), 1);
  if (rows.length === 0) {
    return <span className="empty-state">{empty}</span>;
  }
  return (
    <div className="bar-list">
      {rows.map((row) => (
        <div className="bar-row" key={row.label}>
          <div className="bar-label">
            <span>{row.label}</span>
            <strong>{row.value}</strong>
          </div>
          <div className="bar-track">
            <span style={{ width: `${Math.max(6, Math.round((row.value / max) * 100))}%` }} />
          </div>
        </div>
      ))}
    </div>
  );
}

function groupedRows(items, key) {
  const counts = new Map();
  for (const item of items) {
    const label = item[key] || 'unknown';
    counts.set(label, (counts.get(label) || 0) + 1);
  }
  return [...counts.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((left, right) => right.value - left.value);
}

function operationSummary(cache, t) {
  const rows = groupedRows(cache, 'operation');
  const labelMap = {
    search: t.searchOps,
    fetch: t.fetchOps,
  };
  return rows.map((row) => ({
    ...row,
    label: labelMap[row.label] || row.label || t.otherOps,
  }));
}

function FetchView({
  t,
  body,
  setBody,
  source,
  setSource,
  format,
  setFormat,
  busy,
  error,
  result,
  searchResults,
  searchWarnings,
  selectedResult,
  resultDetail,
  resultDetailBusy,
  searchLyric,
  fetchAggregate,
  openResultDetail,
  closeResultDetail,
  fetchSelectedResult,
  refreshMeta,
}) {
  const hasSearchInput = [body.query, body.title, body.artist, body.id].some((value) => String(value || '').trim());
  const canAggregate = !source && String(body.query || body.title || body.artist || '').trim();

  return (
    <section className="panel">
      <div className="panel-title">
        <h2>{t.fetch}</h2>
        <button type="button" onClick={refreshMeta} title={t.refresh}>
          <RefreshCw size={18} />
        </button>
      </div>
      <form className="fetch-form" onSubmit={searchLyric}>
        <div className="search-fields">
          <input
            value={body.query}
            onChange={(event) => setBody({ ...body, query: event.target.value })}
            placeholder={t.keywordSearch}
          />
          <input
            value={body.title}
            onChange={(event) => setBody({ ...body, title: event.target.value })}
            placeholder={t.titleSearch}
          />
          <input
            value={body.artist}
            onChange={(event) => setBody({ ...body, artist: event.target.value })}
            placeholder={t.artistSearch}
          />
          <input
            value={body.id}
            onChange={(event) => setBody({ ...body, id: event.target.value })}
            placeholder={t.idSearch}
          />
        </div>
        <div className="controls">
          <select value={source} onChange={(event) => setSource(event.target.value)}>
            {sourceOptions.map((option) => (
              <option value={option} key={option || 'aggregate'}>
                {option || t.aggregate}
              </option>
            ))}
          </select>
          <select value={format} onChange={(event) => setFormat(event.target.value)} disabled={!source}>
            <option value="">{t.format}</option>
            <option value="json">{t.json}</option>
            <option value="raw">{t.raw}</option>
          </select>
          <select
            value={body.merge_mode}
            onChange={(event) => setBody({ ...body, merge_mode: event.target.value })}
            disabled={Boolean(source)}
          >
            <option value="tracks">{t.tracks}</option>
            <option value="inline">{t.inline}</option>
          </select>
          <input
            aria-label={t.top}
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
          <input
            aria-label={t.ttl}
            type="number"
            min="60"
            value={body.ttl_seconds}
            onChange={(event) => setBody({ ...body, ttl_seconds: Number(event.target.value) })}
          />
          <label className="check">
            <input
              type="checkbox"
              checked={Boolean(body.force)}
              onChange={(event) => setBody({ ...body, force: event.target.checked })}
            />
            {t.force}
          </label>
          <button type="submit" disabled={busy || !hasSearchInput}>
            <Search size={18} />
            {busy ? t.fetching : t.searchLyrics}
          </button>
          <button type="button" disabled={busy || !canAggregate} onClick={fetchAggregate}>
            <Sparkles size={18} />
            {t.aggregateFetch}
          </button>
        </div>
      </form>
      {error ? <pre className="error">{error}</pre> : null}
      <SearchResultsList
        t={t}
        results={searchResults}
        warnings={searchWarnings}
        openResultDetail={openResultDetail}
      />
      {result ? <pre className="result">{result}</pre> : null}
      <ResultDialog
        t={t}
        entry={selectedResult}
        detail={resultDetail}
        busy={resultDetailBusy}
        close={closeResultDetail}
        fetchSelectedResult={fetchSelectedResult}
      />
    </section>
  );
}

function SearchResultsList({ t, results, warnings, openResultDetail }) {
  return (
    <div className="search-results-panel">
      <div className="search-results-title">
        <div>
          <h3>{t.searchResults}</h3>
          <span>{results.length} {t.candidates}</span>
        </div>
      </div>
      {warnings.length > 0 ? (
        <details className="warning-list">
          <summary>{t.warnings}: {warnings.length}</summary>
          <ul>
            {warnings.slice(0, 8).map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        </details>
      ) : null}
      {results.length === 0 ? (
        <span className="empty-state">{t.noResults}</span>
      ) : (
        <div className="result-table">
          <div className="result-row result-row-head">
            <strong>{t.title}</strong>
            <span>{t.artist}</span>
            <span>{t.source}</span>
            <span>{t.itemId}</span>
            <span>{t.duration}</span>
          </div>
          {results.map((entry) => (
            <button
              type="button"
              className="result-row"
              key={`${entry.source}:${entry.id}:${entry.title}`}
              onClick={() => openResultDetail(entry)}
              title={t.openDetail}
            >
              <strong>{entry.title || '-'}</strong>
              <span>{entry.artist || '-'}</span>
              <span>{displaySource(entry)}</span>
              <span>{entry.id}</span>
              <span>{formatDurationMs(entry.duration_ms)}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function ResultDialog({ t, entry, detail, busy, close, fetchSelectedResult }) {
  if (!entry) {
    return null;
  }
  const aggregate = isAggregateResult(entry);
  const aggregateSources = aggregateMembers(entry)
    .map((member) => displaySource(member))
    .filter((source, index, sources) => source && sources.indexOf(source) === index);

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={close}>
      <section
        className="result-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t.selectedCandidate}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="dialog-title">
          <div>
            <span>{t.selectedCandidate}</span>
            <h3>{entry.title || '-'}</h3>
            <p>{[entry.artist, displaySource(entry), entry.id].filter(Boolean).join(' / ')}</p>
          </div>
          <button type="button" onClick={close} title="Close">
            <X size={18} />
          </button>
        </div>
        <div className="dialog-actions">
          <button type="button" onClick={() => fetchSelectedResult('raw')} disabled={busy || aggregate}>
            {t.fetchRaw}
          </button>
          <button type="button" onClick={() => fetchSelectedResult('json')} disabled={busy}>
            {t.fetchJson}
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
        <div className="detail-section">
          <strong>{busy ? t.fetching : t.detail}</strong>
          <pre className="cache-preview result-preview">{detail || t.resultEmpty}</pre>
        </div>
      </section>
    </div>
  );
}

function CacheView({
  t,
  cache,
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
        <button type="button" onClick={refreshMeta} title={t.refresh}>
          <RefreshCw size={18} />
        </button>
      </div>
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
                <span>{entry.operation}</span>
                <span>{entry.fresh ? t.fresh : t.expired}</span>
                <span>{formatBytes(entry.body_len)}</span>
                <button
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

function formatDurationMs(durationMs) {
  if (!Number.isFinite(durationMs) || durationMs <= 0) {
    return '-';
  }
  const totalSeconds = Math.round(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, '0');
  return `${minutes}:${seconds}`;
}

function displaySource(entry) {
  return entry?.display_source || entry?.extra?.display_source || entry?.source || '-';
}

function isAggregateResult(entry) {
  return entry?.extra?.result_kind === 'aggregate';
}

function aggregateMembers(entry) {
  return Array.isArray(entry?.extra?.aggregate_members) ? entry.extra.aggregate_members : [];
}

function formatTimestamp(timestamp) {
  if (!timestamp) {
    return '-';
  }
  return new Date(timestamp * 1000).toLocaleString();
}

function InspectorView({ t, result }) {
  return (
    <section className="panel split-panel">
      <div>
        <h2>{t.inspector}</h2>
        <p className="hint">{t.qualityPending}</p>
      </div>
      <pre className="result compact">{result || t.resultEmpty}</pre>
    </section>
  );
}

function SettingsView({ t, language, setLanguage, payload }) {
  return (
    <section className="panel split-panel">
      <div className="settings-stack">
        <h2>{t.settings}</h2>
        <label className="field-label">
          {t.language}
          <select value={language} onChange={(event) => setLanguage(event.target.value)}>
            <option value="zh">{t.chinese}</option>
            <option value="en">{t.english}</option>
          </select>
        </label>
        <p className="hint">{t.cachePath}</p>
      </div>
      <pre className="result compact">{JSON.stringify(payload, null, 2)}</pre>
    </section>
  );
}

function Metric({ label, value }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

createRoot(document.getElementById('root')).render(<App />);
