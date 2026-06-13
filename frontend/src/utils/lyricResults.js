import { formatSourceName } from '../lyricPlayback.js';

export function formatDurationMs(durationMs) {
  if (!Number.isFinite(durationMs) || durationMs <= 0) {
    return '-';
  }
  const totalSeconds = Math.round(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, '0');
  return `${minutes}:${seconds}`;
}

export function displaySource(entry) {
  const source = formatSourceName(entry?.display_source || entry?.extra?.display_source || entry?.source);
  return source || '-';
}

export function isAggregateResult(entry) {
  return entry?.extra?.result_kind === 'aggregate';
}

export function sameSearchResult(a, b) {
  return Boolean(a && b && a.id === b.id && displaySource(a) === displaySource(b));
}

export function isQqResult(entry) {
  if (!entry) {
    return false;
  }
  if (isAggregateResult(entry)) {
    return aggregateMembers(entry).some(isQqResult);
  }
  return String(entry.source || entry.extra?.source || entry.extra?.display_source || '').toLowerCase() === 'qq';
}

export function hasSingingAnnotations(entry) {
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

export function preserveEntrySingingAnnotations(data, entry, previousData) {
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

export function mergeEntryExtra(fallbackEntry, entry) {
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

export function aggregateMembers(entry) {
  return Array.isArray(entry?.extra?.aggregate_members) ? entry.extra.aggregate_members : [];
}

export function buildAiScoringPayload(settings) {
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
