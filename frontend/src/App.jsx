import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { createPortal } from 'react-dom';
import { LyricPlaybackView } from './LyricPlaybackView.jsx';
import { defaultLyricSettings, formatSourceName, normalizeLyricPayload, readLyricSettings, resolveLyricGradient } from './lyricPlayback.js';
import {
  BarChart3,
  CheckCircle2,
  Database,
  Languages,
  PanelLeftClose,
  PanelLeftOpen,
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
    mergeMode: '合并模式',
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
    dashboardFocus: '缓存诊断',
    dashboardReady: '待采集',
    dashboardReadyHint: '开始搜索或导入歌词后，这里会汇总缓存质量与来源分布。',
    dashboardFreshHint: '缓存可直接复用',
    dashboardExpiredHint: '建议按需刷新',
    dashboardSourceHint: '来源覆盖',
    dashboardOperationHint: '操作画像',
    dashboardInspectCache: '查看缓存',
    dashboardTopSource: '主要来源',
    dashboardTopOperation: '主要操作',
    dashboardNoSourceHint: '完成一次搜索后会展示各平台占比。',
    dashboardNoOperationHint: '搜索和获取记录会在这里形成操作画像。',
    dashboardNoRecentHint: '最近缓存会以状态列展示大小与新鲜度。',
    dashboardRecentHint: '按响应体大小显示最近缓存，颜色区分新鲜与过期。',
    sourcePolicy: '指定来源必须选择 raw 或 json。',
    advancedOptions: '高级选项',
    searchHint: '输入关键词、歌曲 URL 或平台 ID 开始搜索。也可以填写歌名和艺术家提升匹配率。',
    searchingResults: '正在搜索候选结果…',
    foundResults: '已找到候选结果',
    errorTitle: '操作失败',
    deleteConfirm: '确定删除这条缓存吗？此操作不可撤销。',
    close: '关闭',
    playback: '歌词播放',
    play: '播放',
    pause: '暂停',
    restart: '重播',
    timeline: '时间轴',
    annotations: '助唱标注',
    singingAnnotationTag: '有助唱标注',
    singingAnnotationUnavailableTag: '无助唱标注',
    previousPage: '上一页',
    nextPage: '下一页',
    pageStatus: '第 {page} / {total} 页',
    resultRange: '{start}-{end} / {total}',
    annotationsAvailable: '已包含 QQ 助唱标注',
    annotationsUnavailable: '未发现助唱标注',
    annotationStress: '重音',
    annotationBreath: '换气',
    annotationLongTone: '长音',
    annotationPortamentoUp: '上滑音',
    annotationPortamentoDown: '下滑音',
    rawJson: '原始 JSON',
    lyricPreviewUnavailable: '获取 JSON 后可预览同步歌词。',
    lyricColor: '歌词颜色',
    lyricRenderMode: '歌词显示模式',
    lyricRenderVertical: '竖向滚动',
    lyricRenderKaraoke: '交替双行卡拉 OK',
    lyricStageBackground: '歌词舞台背景',
    lyricTranslationOff: '原文',
    lyricTranslationOnly: '译文',
    lyricTranslationBilingual: '双语',
    lyricColorMode: '颜色模式',
    lyricColorPreset: '颜色预设',
    solidColor: '纯色',
    gradient: '渐变',
    solid: '纯色',
    qqPrism: 'QQ 棱镜',
    aurora: '极光',
    sunset: '日落',
    classic: '经典',
    aiScoring: 'AI 歌词优选',
    aiScoringHint: '聚合歌词时使用 OpenAI 兼容接口为候选打分。',
    aiEnabled: '启用 AI 优选',
    aiBaseUrl: 'OpenAI 兼容基地址',
    aiApiKey: 'API Key',
    aiModel: '模型名',
    aiApiKeyHint: '留空时后端使用 ROSETTRISM_OPENAI_API_KEY。',
    serverToken: '服务 Token',
    serverTokenHint: '远程仪表盘 API 鉴权使用；也可以通过 ?token=... 带入。',
    authFailed: '服务 Token 缺失或无效。',
    collapseSidebar: '收起菜单',
    expandSidebar: '展开菜单',
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
    mergeMode: 'Merge mode',
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
    dashboardFocus: 'Cache diagnostics',
    dashboardReady: 'Ready',
    dashboardReadyHint: 'Search or import lyrics to summarize cache quality and source coverage here.',
    dashboardFreshHint: 'Reusable cache',
    dashboardExpiredHint: 'Refresh as needed',
    dashboardSourceHint: 'Source coverage',
    dashboardOperationHint: 'Operation profile',
    dashboardInspectCache: 'Inspect cache',
    dashboardTopSource: 'Top source',
    dashboardTopOperation: 'Top operation',
    dashboardNoSourceHint: 'Run a search to see platform distribution.',
    dashboardNoOperationHint: 'Search and fetch records will form the operation profile here.',
    dashboardNoRecentHint: 'Recent cache entries will appear as freshness columns.',
    dashboardRecentHint: 'Recent cache entries are sized by response body and colored by freshness.',
    sourcePolicy: 'Source-specific requests require raw or json.',
    advancedOptions: 'Advanced options',
    searchHint: 'Enter keywords, a song URL, or a platform ID to search. Add title and artist for better matches.',
    searchingResults: 'Searching candidates…',
    foundResults: 'Candidate results found',
    errorTitle: 'Operation failed',
    deleteConfirm: 'Delete this cache entry? This cannot be undone.',
    close: 'Close',
    playback: 'Lyric playback',
    play: 'Play',
    pause: 'Pause',
    restart: 'Restart',
    timeline: 'Timeline',
    annotations: 'Singing annotations',
    singingAnnotationTag: 'Has singing annotations',
    singingAnnotationUnavailableTag: 'No singing annotations',
    previousPage: 'Previous',
    nextPage: 'Next',
    pageStatus: 'Page {page} / {total}',
    resultRange: '{start}-{end} / {total}',
    annotationsAvailable: 'QQ singing annotations available',
    annotationsUnavailable: 'No singing annotations found',
    annotationStress: 'Stress',
    annotationBreath: 'Breath',
    annotationLongTone: 'Long tone',
    annotationPortamentoUp: 'Portamento up',
    annotationPortamentoDown: 'Portamento down',
    rawJson: 'Raw JSON',
    lyricPreviewUnavailable: 'Fetch JSON to preview synced lyrics.',
    lyricColor: 'Lyric color',
    lyricRenderMode: 'Lyric display mode',
    lyricRenderVertical: 'Vertical scroll',
    lyricRenderKaraoke: 'Alternating two-line karaoke',
    lyricStageBackground: 'Lyric stage background',
    lyricTranslationOff: 'Original',
    lyricTranslationOnly: 'Translation',
    lyricTranslationBilingual: 'Bilingual',
    lyricColorMode: 'Color mode',
    lyricColorPreset: 'Color preset',
    solidColor: 'Solid color',
    gradient: 'Gradient',
    solid: 'Solid',
    qqPrism: 'QQ prism',
    aurora: 'Aurora',
    sunset: 'Sunset',
    classic: 'Classic',
    aiScoring: 'AI lyric selection',
    aiScoringHint: 'Use an OpenAI-compatible endpoint to score aggregate candidates.',
    aiEnabled: 'Enable AI selection',
    aiBaseUrl: 'OpenAI-compatible base URL',
    aiApiKey: 'API key',
    aiModel: 'Model',
    aiApiKeyHint: 'Leave empty to use ROSETTRISM_OPENAI_API_KEY on the server.',
    serverToken: 'Server token',
    serverTokenHint: 'Used for remote dashboard API requests; ?token=... also works.',
    authFailed: 'Server token is missing or invalid.',
    collapseSidebar: 'Collapse menu',
    expandSidebar: 'Expand menu',
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

const defaultAiSettings = {
  enabled: false,
  baseUrl: '',
  apiKey: '',
  model: '',
};

const SERVER_TOKEN_STORAGE_KEY = 'rosettrism-server-token';
const SERVER_TOKEN_QUERY_KEYS = ['token', 'rosettrism_token', 'server_token'];

function readAiSettings() {
  try {
    const stored = JSON.parse(localStorage.getItem('rosettrism-ai-settings') || 'null');
    const { apiKey: _apiKey, ...storedWithoutSecret } = stored || {};
    return { ...defaultAiSettings, ...storedWithoutSecret };
  } catch {
    return defaultAiSettings;
  }
}

function persistAiSettings(settings) {
  const { apiKey: _apiKey, ...settingsWithoutSecret } = settings;
  localStorage.setItem('rosettrism-ai-settings', JSON.stringify(settingsWithoutSecret));
}

function readServerToken() {
  try {
    const params = new URLSearchParams(window.location.search);
    const key = SERVER_TOKEN_QUERY_KEYS.find((name) => params.get(name)?.trim());
    if (key) {
      const token = params.get(key).trim();
      SERVER_TOKEN_QUERY_KEYS.forEach((name) => params.delete(name));
      sessionStorage.setItem(SERVER_TOKEN_STORAGE_KEY, token);
      const query = params.toString();
      window.history.replaceState(
        null,
        '',
        `${window.location.pathname}${query ? `?${query}` : ''}${window.location.hash}`,
      );
      return token;
    }
    return sessionStorage.getItem(SERVER_TOKEN_STORAGE_KEY) || '';
  } catch {
    return '';
  }
}

function App() {
  const [language, setLanguage] = useState(localStorage.getItem('rosettrism-language') || 'zh');
  const [sidebarCollapsed, setSidebarCollapsed] = useState(localStorage.getItem('rosettrism-sidebar') === 'collapsed');
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
  const [resultDetailData, setResultDetailData] = useState(null);
  const [resultDetailBusy, setResultDetailBusy] = useState(false);
  const [resultDetailScrollTick, setResultDetailScrollTick] = useState(0);
  const [lyricSettings, setLyricSettings] = useState(readLyricSettings);
  const [aiSettings, setAiSettings] = useState(readAiSettings);
  const [serverToken, setServerToken] = useState(readServerToken);
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

  useEffect(() => {
    localStorage.setItem('rosettrism-sidebar', sidebarCollapsed ? 'collapsed' : 'expanded');
  }, [sidebarCollapsed]);

  useEffect(() => {
    localStorage.setItem('rosettrism-lyric-settings', JSON.stringify(lyricSettings));
  }, [lyricSettings]);

  useEffect(() => {
    persistAiSettings(aiSettings);
  }, [aiSettings]);

  useEffect(() => {
    if (serverToken.trim()) {
      sessionStorage.setItem(SERVER_TOKEN_STORAGE_KEY, serverToken.trim());
    } else {
      sessionStorage.removeItem(SERVER_TOKEN_STORAGE_KEY);
    }
  }, [serverToken]);

  const aiScoringPayload = useMemo(() => buildAiScoringPayload(aiSettings), [aiSettings]);

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

  function apiFetch(url, options = {}) {
    const headers = { ...(options.headers || {}) };
    if (serverToken.trim()) {
      headers.authorization = `Bearer ${serverToken.trim()}`;
    }
    return fetch(url, { ...options, headers });
  }

  async function refreshMeta() {
    try {
      const [healthRes, statsRes, cacheRes] = await Promise.all([
        apiFetch('/api/health'),
        apiFetch('/api/stats'),
        apiFetch('/api/cache'),
      ]);
      if (!healthRes.ok || !statsRes.ok || !cacheRes.ok) {
        if ([healthRes, statsRes, cacheRes].some((response) => response.status === 401)) {
          setError(t.authFailed);
        }
        return;
      }
      const [healthData, statsData, cacheData] = await Promise.all([
        readJsonResponse(healthRes),
        readJsonResponse(statsRes),
        readJsonResponse(cacheRes),
      ]);
      setHealth(healthData);
      setStats(statsData);
      const entries = cacheData.entries || [];
      setCache(entries);
      setSelectedCacheEntry((selected) => {
        if (!selected) {
          return selected;
        }
        return entries.find((entry) => entry.id === selected.id) || selected;
      });
    } catch {
      return;
    }
  }

  async function searchLyric(event) {
    event.preventDefault();
    setBusy(true);
    setError('');
    setResult('');
    setSearchResults([]);
    setSearchWarnings([]);
    setSelectedResult(null);
    setResultDetail('');
    setResultDetailData(null);
    setResultDetailBusy(false);
    try {
      const response = await apiFetch('/api/search', {
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
      const response = await apiFetch('/api/fetch', {
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
    setError('');
    try {
      const response = await apiFetch('/api/fetch-result', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          result: selectedResult,
          format: requestedFormat,
          ttl_seconds: body.ttl_seconds,
          force: Boolean(body.force),
          ...(aiScoringPayload ? { ai_scoring: aiScoringPayload } : {}),
        }),
      });
      const text = await response.text();
      if (!response.ok) {
        throw new Error(text);
      }
      const data = preserveEntrySingingAnnotations(JSON.parse(text), selectedResult, resultDetailData);
      setResultDetailData(data);
      setResultDetail(JSON.stringify(data, null, 2));
      if (requestedFormat === 'json') {
        setResultDetailScrollTick((tick) => tick + 1);
      }
      await refreshMeta();
    } catch (err) {
      setResultDetailData(null);
      setResultDetail(err.message);
    } finally {
      setResultDetailBusy(false);
    }
  }

  async function deleteCache(id) {
    if (!window.confirm(t.deleteConfirm)) {
      return;
    }
    setError('');
    try {
      const response = await apiFetch(`/api/cache/${id}`, { method: 'DELETE' });
      if (!response.ok) {
        throw new Error(await response.text());
      }
      if (selectedCacheEntry?.id === id) {
        setSelectedCacheEntry(null);
        setCacheDetail(null);
      }
      await refreshMeta();
    } catch (err) {
      setError(err.message);
    }
  }

  async function selectCacheEntry(entry) {
    setSelectedCacheEntry(entry);
    setCacheDetail(null);
    setCacheDetailBusy(true);
    try {
      const response = await apiFetch(`/api/cache/${entry.id}`);
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

function Overview({ t, stats, cache, setActiveView }) {
  const total = stats?.cache?.upstream_entries ?? cache.length;
  const fresh = stats?.cache?.fresh_upstream_entries ?? cache.filter((entry) => entry.fresh).length;
  const expired = Math.max((stats?.cache?.expired_upstream_entries ?? total - fresh) || 0, 0);
  const freshPercent = total > 0 ? Math.round((fresh / total) * 100) : 0;
  const sourceRows = groupedRows(cache, 'source').slice(0, 4);
  const operationRows = operationSummary(cache, t);
  const recentRows = cache.slice(0, 8);
  const topSource = sourceRows[0]?.label || '-';
  const topOperation = operationRows[0]?.label || '-';

  return (
    <section className="dashboard-grid">
      <article className="dashboard-brief">
        <div>
          <span className="dashboard-kicker">{t.dashboardFocus}</span>
          <h2>{total > 0 ? `${freshPercent}% ${t.freshRatio}` : t.dashboardReady}</h2>
          <p>{total > 0 ? t.dashboardRecentHint : t.dashboardReadyHint}</p>
        </div>
        <div className="dashboard-brief-stats">
          <MiniStat label={t.totalEntries} value={total} />
          <MiniStat label={t.dashboardTopSource} value={topSource} />
          <MiniStat label={t.dashboardTopOperation} value={topOperation} />
        </div>
      </article>

      <article className="chart-panel health-panel">
        <div className="chart-title">
          <div>
            <h2>{t.cacheHealth}</h2>
            <p>{t.totalEntries}: {total}</p>
          </div>
          <CheckCircle2 size={20} />
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
            <LegendDot label={t.fresh} value={fresh} color="green" hint={t.dashboardFreshHint} />
            <LegendDot label={t.expired} value={expired} color="amber" hint={t.dashboardExpiredHint} />
            <button className="button-secondary text-action" type="button" onClick={() => setActiveView('cache')}>
              <Database size={16} />
              {t.dashboardInspectCache}
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
        <BarList rows={sourceRows} emptyTitle={t.noData} emptyHint={t.dashboardNoSourceHint} footer={t.dashboardSourceHint} />
      </article>

      <article className="chart-panel">
        <div className="chart-title">
          <div>
            <h2>{t.operationMix}</h2>
            <p>{t.searchOps} / {t.fetchOps}</p>
          </div>
          <Server size={20} />
        </div>
        <BarList rows={operationRows} emptyTitle={t.noData} emptyHint={t.dashboardNoOperationHint} footer={t.dashboardOperationHint} />
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
            <EmptyState title={t.noData} hint={t.dashboardNoRecentHint} />
          ) : (
            recentRows.map((entry) => (
              <div className="spark-column" key={entry.id}>
                <span
                  className={entry.fresh ? 'spark-bar fresh-bar' : 'spark-bar expired-bar'}
                  style={{ height: `${Math.max(16, Math.min(58, Math.round((entry.body_len || 1) / 120)))}px` }}
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

function MiniStat({ label, value }) {
  return (
    <div className="mini-stat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function LegendDot({ label, value, color, hint }) {
  return (
    <div className="legend-row">
      <span className={`legend-dot ${color}`} />
      <span>
        {label}
        {hint ? <small>{hint}</small> : null}
      </span>
      <strong>{value}</strong>
    </div>
  );
}

function BarList({ rows, emptyTitle, emptyHint, footer }) {
  const max = Math.max(...rows.map((row) => row.value), 1);
  if (rows.length === 0) {
    return <EmptyState title={emptyTitle} hint={emptyHint} />;
  }
  return (
    <>
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
      {footer ? <span className="chart-footnote">{footer}</span> : null}
    </>
  );
}

function EmptyState({ title, hint }) {
  return (
    <div className="empty-state rich-empty-state">
      <span>{title}</span>
      {hint ? <small>{hint}</small> : null}
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
}) {
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
                  <option value={option} key={option || 'aggregate'}>
                    {option || t.aggregate}
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
      {error ? <ErrorMessage t={t} error={error} /> : null}
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

function ErrorMessage({ t, error }) {
  return (
    <div className="error" role="alert">
      <strong>{t.errorTitle}</strong>
      <pre>{error}</pre>
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
          <button className="button-primary" type="button" onClick={() => fetchSelectedResult('json')} disabled={busy}>
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
        <button className="button-icon" type="button" onClick={refreshMeta} title={t.refresh} aria-label={t.refresh}>
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

async function readJsonResponse(response) {
  const contentType = response.headers.get('content-type') || '';
  if (!contentType.includes('application/json')) {
    throw new Error('Expected JSON response');
  }
  return response.json();
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
  const source = formatSourceName(entry?.display_source || entry?.extra?.display_source || entry?.source);
  return source || '-';
}

function isAggregateResult(entry) {
  return entry?.extra?.result_kind === 'aggregate';
}

function sameSearchResult(a, b) {
  return Boolean(a && b && a.id === b.id && displaySource(a) === displaySource(b));
}

function isQqResult(entry) {
  if (!entry) {
    return false;
  }
  if (isAggregateResult(entry)) {
    return aggregateMembers(entry).some(isQqResult);
  }
  return String(entry.source || entry.extra?.source || entry.extra?.display_source || '').toLowerCase() === 'qq';
}

function hasSingingAnnotations(entry) {
  if (!entry) {
    return false;
  }
  if (isAggregateResult(entry)) {
    return aggregateMembers(entry).some(hasSingingAnnotations);
  }
  const extra = entry.extra || {};
  return [
    extra.has_singing_annotations,
    extra.hasSingingAnnotations,
    extra.hasSingingAnnotationsLyric,
    extra.singing_annotations,
    extra.singingAnnotationsLyric,
  ].some(hasPositiveAnnotationSignal);
}

function hasPositiveAnnotationSignal(value) {
  if (Array.isArray(value)) {
    return value.length > 0;
  }
  return value === true || value === 1 || value === '1' || String(value).toLowerCase() === 'true';
}

function preserveEntrySingingAnnotations(data, entry, previousData) {
  const annotations = pickSingingAnnotationsFromSources(
    data?.selectedEntry,
    data?.result,
    data,
    previousData?.selectedEntry,
    previousData?.result,
    previousData,
    entry,
  );
  if (!annotations.length) {
    return data;
  }
  const selectedEntry = mergeEntryExtra(entry, data?.selectedEntry || data?.result || entry);
  const nextData = {
    ...data,
    selectedEntry: {
      ...selectedEntry,
      extra: withSingingAnnotations(selectedEntry?.extra, annotations),
    },
  };
  if (data?.result && data.result !== data.selectedEntry) {
    nextData.result = {
      ...data.result,
      extra: withSingingAnnotations(data.result.extra, annotations),
    };
  }
  return nextData;
}

function mergeEntryExtra(fallbackEntry, entry) {
  if (!fallbackEntry && !entry) {
    return entry;
  }
  return {
    ...(fallbackEntry || {}),
    ...(entry || {}),
    extra: {
      ...(fallbackEntry?.extra || {}),
      ...(entry?.extra || {}),
    },
  };
}

function pickSingingAnnotationsFromSources(...sources) {
  for (const source of sources) {
    const annotations = pickSingingAnnotations(source?.extra || source || {});
    if (annotations.length) {
      return annotations;
    }
  }
  return [];
}

function pickSingingAnnotations(extra) {
  return [
    extra.singing_annotations,
    extra.singingAnnotationsLyric,
    extra.singingAnnotations,
    extra.hasSingingAnnotationsLyric,
    extra.qq_singing_annotations,
    extra.qqSingingAnnotations,
  ].find((value) => Array.isArray(value) && value.length) || [];
}

function withSingingAnnotations(extra = {}, annotations) {
  const existing = pickSingingAnnotations(extra);
  const mergedAnnotations = existing.length ? existing : annotations;
  return {
    ...extra,
    has_singing_annotations: extra.has_singing_annotations || true,
    hasSingingAnnotations: extra.hasSingingAnnotations || true,
    singing_annotations: mergedAnnotations,
    singingAnnotationsLyric: Array.isArray(extra.singingAnnotationsLyric) && extra.singingAnnotationsLyric.length ? extra.singingAnnotationsLyric : mergedAnnotations,
  };
}

function aggregateMembers(entry) {
  return Array.isArray(entry?.extra?.aggregate_members) ? entry.extra.aggregate_members : [];
}

function buildAiScoringPayload(settings) {
  if (!settings?.enabled) {
    return null;
  }
  const payload = { enabled: true };
  if (settings.baseUrl?.trim()) {
    payload.base_url = settings.baseUrl.trim();
  }
  if (settings.apiKey?.trim()) {
    payload.api_key = settings.apiKey.trim();
  }
  if (settings.model?.trim()) {
    payload.model = settings.model.trim();
  }
  return payload;
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

function SettingsView({
  t,
  language,
  setLanguage,
  lyricSettings,
  setLyricSettings,
  aiSettings,
  setAiSettings,
  serverToken,
  setServerToken,
  payload,
}) {
  const previewStyle = {
    '--lyric-solid-color': lyricSettings.solidColor,
    '--lyric-gradient': resolveLyricGradient(lyricSettings.colorPreset),
    '--lyric-stage-background': lyricSettings.stageBackgroundColor || defaultLyricSettings.stageBackgroundColor,
  };

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
        <label className="field-label">
          {t.serverToken}
          <input
            type="password"
            value={serverToken}
            autoComplete="off"
            onChange={(event) => setServerToken(event.target.value)}
          />
          <span>{t.serverTokenHint}</span>
        </label>
        <div className="settings-group">
          <strong>{t.aiScoring}</strong>
          <p className="hint">{t.aiScoringHint}</p>
          <label className="field-label settings-checkbox-row">
            <input
              type="checkbox"
              checked={Boolean(aiSettings.enabled)}
              onChange={(event) => setAiSettings({ ...aiSettings, enabled: event.target.checked })}
            />
            {t.aiEnabled}
          </label>
          <label className="field-label">
            {t.aiBaseUrl}
            <input
              type="url"
              value={aiSettings.baseUrl}
              placeholder="https://api.openai.com/v1"
              onChange={(event) => setAiSettings({ ...aiSettings, baseUrl: event.target.value })}
            />
          </label>
          <label className="field-label">
            {t.aiModel}
            <input
              type="text"
              value={aiSettings.model}
              placeholder="gpt-4o-mini"
              onChange={(event) => setAiSettings({ ...aiSettings, model: event.target.value })}
            />
          </label>
          <label className="field-label">
            {t.aiApiKey}
            <input
              type="password"
              value={aiSettings.apiKey}
              autoComplete="off"
              placeholder="sk-..."
              onChange={(event) => setAiSettings({ ...aiSettings, apiKey: event.target.value })}
            />
            <span>{t.aiApiKeyHint}</span>
          </label>
        </div>
        <div className="settings-group">
          <strong>{t.lyricColor}</strong>
          <label className="field-label">
            {t.lyricRenderMode}
            <select
              value={lyricSettings.renderMode || defaultLyricSettings.renderMode}
              onChange={(event) => setLyricSettings({ ...lyricSettings, renderMode: event.target.value })}
            >
              <option value="vertical">{t.lyricRenderVertical}</option>
              <option value="karaoke">{t.lyricRenderKaraoke}</option>
            </select>
          </label>
          <label className="field-label settings-color-row">
            {t.lyricStageBackground}
            <input
              type="color"
              value={lyricSettings.stageBackgroundColor || defaultLyricSettings.stageBackgroundColor}
              onChange={(event) => setLyricSettings({ ...lyricSettings, stageBackgroundColor: event.target.value })}
            />
          </label>
          <label className="field-label">
            {t.lyricColorMode}
            <select
              value={lyricSettings.colorMode}
              onChange={(event) => setLyricSettings({ ...lyricSettings, colorMode: event.target.value })}
            >
              <option value="gradient">{t.gradient}</option>
              <option value="solid">{t.solid}</option>
            </select>
          </label>
          <label className="field-label">
            {t.lyricColorPreset}
            <select
              value={lyricSettings.colorPreset}
              disabled={lyricSettings.colorMode !== 'gradient'}
              onChange={(event) => setLyricSettings({ ...lyricSettings, colorPreset: event.target.value })}
            >
              <option value="qq-prism">{t.qqPrism}</option>
              <option value="aurora">{t.aurora}</option>
              <option value="sunset">{t.sunset}</option>
              <option value="classic">{t.classic}</option>
            </select>
          </label>
          <label className="field-label settings-color-row">
            {t.solidColor}
            <input
              type="color"
              value={lyricSettings.solidColor || defaultLyricSettings.solidColor}
              disabled={lyricSettings.colorMode !== 'solid'}
              onChange={(event) => setLyricSettings({ ...lyricSettings, solidColor: event.target.value })}
            />
          </label>
          <div className={`lyric-color-preview lyric-color-${lyricSettings.colorMode}`} style={previewStyle}>
            Rosettrism Lyrics
          </div>
        </div>
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
