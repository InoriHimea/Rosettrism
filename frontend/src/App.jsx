import { useCallback, useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import {
  BarChart3,
  Database,
  Languages,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  Server,
  Settings,
  Sparkles,
} from 'lucide-react';
import { createApiClient, readServerToken, writeServerToken } from './api/client.js';
import { formatApiErrorForDisplay } from './api/errors.js';
import { useAiSettings } from './hooks/useAiSettings.js';
import { useCacheEntries } from './hooks/useCacheEntries.js';
import { useLyricSettings } from './hooks/useLyricSettings.js';
import { useSidebarState } from './hooks/useSidebarState.js';
import { dictionaries } from './i18n/dictionaries.js';
import { CacheView } from './views/CacheView.jsx';
import { FetchView } from './views/FetchView.jsx';
import { InspectorView } from './views/InspectorView.jsx';
import { OverviewView } from './views/OverviewView.jsx';
import { SettingsView } from './views/SettingsView.jsx';
import { preserveEntrySingingAnnotations, sameSearchResult } from './utils/lyricResults.js';
import './styles.css';

const navItems = [
  ['overview', BarChart3],
  ['fetch', Search],
  ['cache', Database],
  ['inspector', Sparkles],
  ['settings', Settings],
];

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
  const { sidebarCollapsed, setSidebarCollapsed } = useSidebarState();
  const [activeView, setActiveView] = useState('overview');
  const [body, setBody] = useState(defaultBody);
  const [source, setSource] = useState('');
  const [format, setFormat] = useState('');
  const [result, setResult] = useState('');
  const [searchResults, setSearchResults] = useState([]);
  const [searchWarnings, setSearchWarnings] = useState([]);
  const [selectedResult, setSelectedResult] = useState(null);
  const [resultDetail, setResultDetail] = useState('');
  const [resultDetailData, setResultDetailData] = useState(null);
  const [resultDetailBusy, setResultDetailBusy] = useState(false);
  const [resultDetailScrollTick, setResultDetailScrollTick] = useState(0);
  const { lyricSettings, setLyricSettings } = useLyricSettings();
  const { aiSettings, setAiSettings, aiScoringPayload } = useAiSettings();
  const [serverToken, setServerToken] = useState(readServerToken);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(null);
  const t = dictionaries[language] || dictionaries.zh;
  const apiClient = useMemo(() => createApiClient(serverToken), [serverToken]);
  const {
    health,
    stats,
    cache,
    unifiedCache,
    selectedCacheEntry,
    cacheDetail,
    cacheDetailBusy,
    refreshMeta,
    selectCacheEntry,
    deleteCache,
  } = useCacheEntries(apiClient, t);

  useEffect(() => {
    refreshMeta();
  }, [refreshMeta]);

  useEffect(() => {
    localStorage.setItem('rosettrism-language', language);
  }, [language]);

  useEffect(() => {
    writeServerToken(serverToken);
  }, [serverToken]);

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
    if (aiScoringPayload) {
      next.ai_scoring = aiScoringPayload;
    }
    return next;
  }, [body, aiScoringPayload]);

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

  const searchLyric = useCallback(async (event) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    setResult('');
    setSearchResults([]);
    setSearchWarnings([]);
    try {
      const data = await apiClient.postJson('/api/search', searchPayload);
      setSearchResults(data.results || []);
      setSearchWarnings(data.warnings || []);
      setActiveView('fetch');
      await refreshMeta();
    } catch (err) {
      setError(err);
    } finally {
      setBusy(false);
    }
  }, [apiClient, refreshMeta, searchPayload]);

  const fetchAggregate = useCallback(async () => {
    setBusy(true);
    setError(null);
    setResult('');
    try {
      const data = await apiClient.postJson('/api/fetch', aggregatePayload);
      setResult(JSON.stringify(data, null, 2));
      setActiveView('fetch');
      await refreshMeta();
    } catch (err) {
      setError(err);
    } finally {
      setBusy(false);
    }
  }, [aggregatePayload, apiClient, refreshMeta]);

  function openResultDetail(entry) {
    const sameResult = sameSearchResult(entry, selectedResult) || sameSearchResult(entry, resultDetailData?.result);
    setSelectedResult(entry);
    if (!sameResult) {
      setResultDetail('');
      setResultDetailData(null);
    }
  }

  function closeResultDetail() {
    setSelectedResult(null);
    setResultDetailBusy(false);
  }

  async function fetchSelectedResult(requestedFormat) {
    if (!selectedResult) {
      return;
    }
    setResultDetailBusy(true);
    setError(null);
    try {
      const data = await apiClient.postJson('/api/fetch-result', {
        result: selectedResult,
        format: requestedFormat,
        ttl_seconds: body.ttl_seconds,
        force: Boolean(body.force),
        ...(aiScoringPayload ? { ai_scoring: aiScoringPayload } : {}),
      });
      const dataWithAnnotations = preserveEntrySingingAnnotations(data, selectedResult, resultDetailData);
      setResultDetailData(dataWithAnnotations);
      setResultDetail(JSON.stringify(dataWithAnnotations, null, 2));
      if (requestedFormat === 'json') {
        setResultDetailScrollTick((tick) => tick + 1);
      }
      await refreshMeta();
    } catch (err) {
      setResultDetailData(null);
      setResultDetail(formatApiErrorForDisplay(err, t));
    } finally {
      setResultDetailBusy(false);
    }
  }

  return (
    <main className={`app-shell ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
      <aside className="sidebar">
        <div className="brand-row">
          <div className="brand">
            <LogoMark />
            <div className="brand-copy">
              <h1>Rosettrism</h1>
              <p>{t.dashboard}</p>
            </div>
          </div>
          <button
            className="sidebar-toggle button-icon"
            type="button"
            onClick={() => setSidebarCollapsed((collapsed) => !collapsed)}
            aria-label={sidebarCollapsed ? t.expandSidebar : t.collapseSidebar}
            title={sidebarCollapsed ? t.expandSidebar : t.collapseSidebar}
          >
            {sidebarCollapsed ? <PanelLeftOpen size={18} /> : <PanelLeftClose size={18} />}
          </button>
        </div>
        <nav aria-label="Dashboard">
          {navItems.map(([id, Icon]) => (
            <button
              type="button"
              className={activeView === id ? 'nav-active' : ''}
              onClick={() => setActiveView(id)}
              key={id}
              title={t[id]}
            >
              <Icon size={18} />
              <span>{t[id]}</span>
            </button>
          ))}
        </nav>
        <button className="language-button" type="button" onClick={() => setLanguage(language === 'zh' ? 'en' : 'zh')} title={language === 'zh' ? t.english : t.chinese}>
          <Languages size={18} />
          <span>{language === 'zh' ? t.english : t.chinese}</span>
        </button>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <Metric label={t.server} value={health?.ok ? t.online : t.checking} />
          <Metric label={t.version} value={health?.version || '-'} />
          <Metric label={t.upstreamCache} value={stats?.cache?.upstream_entries ?? '-'} />
          <Metric label={t.fresh} value={stats?.cache?.fresh_upstream_entries ?? '-'} />
        </header>

        {activeView === 'overview' && <OverviewView t={t} stats={stats} cache={cache} setActiveView={setActiveView} />}
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
            resultDetailData={resultDetailData}
            resultDetailBusy={resultDetailBusy}
            lyricSettings={lyricSettings}
            resultDetailScrollTick={resultDetailScrollTick}
            searchLyric={searchLyric}
            fetchAggregate={fetchAggregate}
            openResultDetail={openResultDetail}
            closeResultDetail={closeResultDetail}
            fetchSelectedResult={fetchSelectedResult}
            refreshMeta={refreshMeta}
            setActiveView={setActiveView}
          />
        )}
        {activeView === 'cache' && (
          <CacheView
            t={t}
            cache={cache}
            unifiedCache={unifiedCache}
            stats={stats}
            selectedCacheEntry={selectedCacheEntry}
            cacheDetail={cacheDetail}
            cacheDetailBusy={cacheDetailBusy}
            selectCacheEntry={selectCacheEntry}
            deleteCache={deleteCache}
            refreshMeta={refreshMeta}
          />
        )}
        {activeView === 'inspector' && <InspectorView t={t} result={result} stats={stats} />}
        {activeView === 'settings' && (
          <SettingsView
            t={t}
            language={language}
            setLanguage={setLanguage}
            lyricSettings={lyricSettings}
            setLyricSettings={setLyricSettings}
            aiSettings={aiSettings}
            setAiSettings={setAiSettings}
            serverToken={serverToken}
            setServerToken={setServerToken}
            payload={searchPayload}
          />
        )}
      </section>
    </main>
  );
}

function LogoMark() {
  return (
    <div className="logo-mark" aria-hidden="true">
      <svg viewBox="0 0 48 48" role="img">
        <defs>
          <linearGradient id="logoGradient" x1="8" x2="40" y1="6" y2="42" gradientUnits="userSpaceOnUse">
            <stop stopColor="#7c3aed" />
            <stop offset="0.48" stopColor="#06b6d4" />
            <stop offset="1" stopColor="#22c55e" />
          </linearGradient>
        </defs>
        <path d="M24 4 42 14.4v19.2L24 44 6 33.6V14.4L24 4Z" fill="url(#logoGradient)" />
        <path d="M15 29c5.4-8.8 12.6-8.8 18 0" fill="none" stroke="white" strokeLinecap="round" strokeWidth="4" />
        <path d="M15 19h18" stroke="white" strokeLinecap="round" strokeWidth="4" />
        <circle cx="16" cy="19" r="3" fill="#fef08a" />
        <circle cx="32" cy="29" r="3" fill="#fef08a" />
      </svg>
    </div>
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
