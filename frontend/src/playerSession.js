export const PLAYER_SESSION_VERSION = 1;
export const PLAYER_SESSION_STORAGE_KEY = 'rosettrism-player-session';
export const PLAYBACK_MODES = Object.freeze(['order', 'repeat-one', 'repeat-all', 'shuffle']);

const DEFAULT_VOLUME = 1;
const DEFAULT_RATE = 1;
const DEFAULT_MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000;

function finite(value, fallback = 0) {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : fallback;
}

function clamp(value, min, max, fallback = min) {
  return Math.max(min, Math.min(max, finite(value, fallback)));
}

function normalizeMode(mode) {
  return PLAYBACK_MODES.includes(mode) ? mode : 'order';
}

export function normalizeQueueItem(item, index = 0) {
  if (!item || typeof item !== 'object') {
    return null;
  }
  const src = String(item.src || item.audioSrc || item.url || '').trim();
  if (!src) {
    return null;
  }
  const id = String(item.id || item.trackId || `track-${index}-${src}`).trim();
  if (!id) {
    return null;
  }
  const expiresAt = item.expiresAt == null ? null : finite(item.expiresAt, 0);
  return {
    id,
    src,
    title: String(item.title || item.name || 'Untitled').trim() || 'Untitled',
    artist: String(item.artist || '').trim(),
    album: String(item.album || '').trim(),
    artwork: typeof item.artwork === 'string' ? item.artwork : null,
    lyric: item.lyric && typeof item.lyric === 'object' ? item.lyric : null,
    durable: item.durable === true,
    expiresAt: expiresAt > 0 ? expiresAt : null,
  };
}

export function normalizeQueue(queue) {
  const seen = new Set();
  return (Array.isArray(queue) ? queue : [])
    .map(normalizeQueueItem)
    .filter((item) => {
      if (!item || seen.has(item.id)) {
        return false;
      }
      seen.add(item.id);
      return true;
    });
}

function currentIndexFor(queue, currentId, fallback = 0) {
  if (!queue.length) {
    return -1;
  }
  const index = queue.findIndex((item) => item.id === currentId);
  return index >= 0 ? index : Math.max(0, Math.min(queue.length - 1, fallback));
}

export function createPlayerSessionState({
  queue = [],
  currentId,
  currentIndex = 0,
  mode = 'order',
  positionMs = 0,
  volume = DEFAULT_VOLUME,
  muted = false,
  playbackRate = DEFAULT_RATE,
  isPlaying = false,
  status = 'idle',
  error = null,
  retryCount = 0,
} = {}) {
  const normalizedQueue = normalizeQueue(queue);
  const index = currentIndexFor(normalizedQueue, currentId, currentIndex);
  return {
    queue: normalizedQueue,
    currentIndex: index,
    currentId: index >= 0 ? normalizedQueue[index].id : null,
    mode: normalizeMode(mode),
    positionMs: Math.max(0, finite(positionMs)),
    volume: clamp(volume, 0, 1, DEFAULT_VOLUME),
    muted: Boolean(muted),
    playbackRate: clamp(playbackRate, 0.25, 4, DEFAULT_RATE),
    isPlaying: Boolean(isPlaying && index >= 0),
    status: String(status || 'idle'),
    error: error ? String(error) : null,
    retryCount: Math.max(0, Math.floor(finite(retryCount))),
  };
}

function indexAfter(state, direction, random) {
  const length = state.queue.length;
  if (!length || state.currentIndex < 0) {
    return -1;
  }
  if (state.mode === 'repeat-one') {
    return state.currentIndex;
  }
  if (state.mode === 'shuffle') {
    if (length === 1) {
      return state.currentIndex;
    }
    const candidates = state.queue.map((_, index) => index).filter((index) => index !== state.currentIndex);
    const pick = Math.floor(clamp(random(), 0, 0.999999, 0) * candidates.length);
    return candidates[pick];
  }
  const next = state.currentIndex + direction;
  if (next >= 0 && next < length) {
    return next;
  }
  return state.mode === 'repeat-all' ? (direction > 0 ? 0 : length - 1) : -1;
}

function selectIndex(state, index, { autoplay = false } = {}) {
  if (index < 0 || index >= state.queue.length) {
    return {
      ...state,
      currentIndex: -1,
      currentId: null,
      positionMs: 0,
      isPlaying: false,
      status: 'idle',
      error: null,
    };
  }
  return {
    ...state,
    currentIndex: index,
    currentId: state.queue[index].id,
    positionMs: 0,
    isPlaying: Boolean(autoplay),
    status: autoplay ? 'loading' : 'ready',
    error: null,
    retryCount: 0,
  };
}

export function reducePlayerSession(state, action, { random = Math.random } = {}) {
  switch (action?.type) {
    case 'set-queue': {
      const queue = normalizeQueue(action.queue);
      const index = currentIndexFor(queue, action.currentId ?? state.currentId, 0);
      return {
        ...createPlayerSessionState({ ...state, queue, currentIndex: index }),
        isPlaying: Boolean(state.isPlaying && index >= 0),
      };
    }
    case 'select':
      return selectIndex(state, currentIndexFor(state.queue, action.id, action.index), { autoplay: action.autoplay });
    case 'next': {
      const index = indexAfter(state, 1, random);
      return index >= 0
        ? selectIndex(state, index, { autoplay: action.autoplay !== false })
        : { ...state, isPlaying: false, status: 'ended' };
    }
    case 'previous': {
      const index = indexAfter(state, -1, random);
      return index >= 0
        ? selectIndex(state, index, { autoplay: action.autoplay !== false })
        : { ...state, isPlaying: false };
    }
    case 'ended': {
      if (state.mode === 'repeat-one') {
        return { ...state, positionMs: 0, isPlaying: action.autoplay !== false, status: 'loading', error: null };
      }
      const index = indexAfter(state, 1, random);
      return index >= 0
        ? selectIndex(state, index, { autoplay: action.autoplay !== false })
        : { ...state, isPlaying: false, status: 'ended', error: null };
    }
    case 'set-mode':
      return { ...state, mode: normalizeMode(action.mode) };
    case 'set-position':
      return { ...state, positionMs: Math.max(0, finite(action.positionMs)), status: state.status === 'ended' ? 'ready' : state.status };
    case 'set-volume':
      return { ...state, volume: clamp(action.volume, 0, 1, state.volume), muted: false };
    case 'set-muted':
      return { ...state, muted: Boolean(action.muted) };
    case 'set-rate':
      return { ...state, playbackRate: clamp(action.playbackRate, 0.25, 4, state.playbackRate) };
    case 'set-playing':
      return { ...state, isPlaying: Boolean(action.isPlaying), status: action.isPlaying ? 'playing' : 'paused' };
    case 'set-status':
      return { ...state, status: String(action.status || 'idle'), error: action.error ? String(action.error) : null };
    case 'media-error':
      return { ...state, isPlaying: false, status: 'error', error: String(action.error || 'MEDIA_ERR_UNKNOWN'), retryCount: state.retryCount + 1 };
    case 'retry':
      return { ...state, isPlaying: false, status: 'loading', error: null };
    case 'clear':
      return createPlayerSessionState();
    default:
      return state;
  }
}

export function createPlayerSession(options = {}) {
  let state = createPlayerSessionState(options);
  const listeners = new Set();
  const random = options.random || Math.random;

  function notify() {
    for (const listener of listeners) {
      listener(state);
    }
  }

  function dispatch(action) {
    state = reducePlayerSession(state, action, { random });
    notify();
    return state;
  }

  return {
    getState: () => state,
    dispatch,
    subscribe(listener) {
      listeners.add(listener);
      listener(state);
      return () => listeners.delete(listener);
    },
    select: (id, autoplay = false) => dispatch({ type: 'select', id, autoplay }),
    next: (autoplay = true) => dispatch({ type: 'next', autoplay }),
    previous: (autoplay = true) => dispatch({ type: 'previous', autoplay }),
    ended: (autoplay = true) => dispatch({ type: 'ended', autoplay }),
    setMode: (mode) => dispatch({ type: 'set-mode', mode }),
    setPosition: (positionMs) => dispatch({ type: 'set-position', positionMs }),
    setVolume: (volume) => dispatch({ type: 'set-volume', volume }),
    setMuted: (muted) => dispatch({ type: 'set-muted', muted }),
    setPlaybackRate: (playbackRate) => dispatch({ type: 'set-rate', playbackRate }),
    setPlaying: (isPlaying) => dispatch({ type: 'set-playing', isPlaying }),
    destroy() {
      listeners.clear();
    },
  };
}

function isPersistable(item, now) {
  return item?.durable === true && (!item.expiresAt || item.expiresAt > now);
}

export function serializePlayerSession(state, { now = Date.now } = {}) {
  const savedAt = now();
  const queue = state.queue
    .filter((item) => isPersistable(item, savedAt))
    .map(({ lyric, ...item }) => item);
  const currentId = queue.some((item) => item.id === state.currentId) ? state.currentId : queue[0]?.id || null;
  return JSON.stringify({
    version: PLAYER_SESSION_VERSION,
    savedAt,
    queue,
    currentId,
    mode: state.mode,
    positionMs: state.positionMs,
    volume: state.volume,
    muted: state.muted,
    playbackRate: state.playbackRate,
  });
}

export function restorePlayerSession(serialized, { now = Date.now, maxAgeMs = DEFAULT_MAX_AGE_MS } = {}) {
  try {
    const payload = typeof serialized === 'string' ? JSON.parse(serialized) : serialized;
    const currentTime = now();
    if (!payload || payload.version !== PLAYER_SESSION_VERSION || !Number.isFinite(payload.savedAt)) {
      return createPlayerSessionState();
    }
    if (currentTime - payload.savedAt > maxAgeMs || currentTime < payload.savedAt) {
      return createPlayerSessionState();
    }
    const queue = normalizeQueue(payload.queue).filter((item) => isPersistable(item, currentTime));
    return createPlayerSessionState({
      queue,
      currentId: payload.currentId,
      mode: payload.mode,
      positionMs: payload.positionMs,
      volume: payload.volume,
      muted: payload.muted,
      playbackRate: payload.playbackRate,
      isPlaying: false,
      status: queue.length ? 'ready' : 'idle',
    });
  } catch {
    return createPlayerSessionState();
  }
}

export function reconcilePlayerSession(restored, queue, { now = Date.now } = {}) {
  const currentTime = now();
  const availableQueue = normalizeQueue(queue).filter((item) => !item.expiresAt || item.expiresAt > currentTime);
  if (!availableQueue.length) {
    return createPlayerSessionState();
  }
  const restoredCurrentId = availableQueue.some((item) => item.id === restored?.currentId)
    ? restored.currentId
    : availableQueue[0].id;
  return createPlayerSessionState({
    queue: availableQueue,
    currentId: restoredCurrentId,
    mode: restored?.mode,
    positionMs: restored?.positionMs,
    volume: restored?.volume,
    muted: restored?.muted,
    playbackRate: restored?.playbackRate,
    isPlaying: false,
    status: 'ready',
  });
}

export function readPersistedPlayerSession({ storage = globalThis.localStorage, now = Date.now, maxAgeMs = DEFAULT_MAX_AGE_MS } = {}) {
  try {
    return restorePlayerSession(storage?.getItem(PLAYER_SESSION_STORAGE_KEY), { now, maxAgeMs });
  } catch {
    return createPlayerSessionState();
  }
}

export function persistPlayerSession(state, { storage = globalThis.localStorage, now = Date.now } = {}) {
  try {
    storage?.setItem(PLAYER_SESSION_STORAGE_KEY, serializePlayerSession(state, { now }));
  } catch {
    // Storage may be disabled or unavailable in private browsing.
  }
}
