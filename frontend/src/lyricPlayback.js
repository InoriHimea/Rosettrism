export const defaultLyricSettings = {
  colorMode: 'gradient',
  colorPreset: 'qq-prism',
  solidColor: '#14c9a2',
  renderMode: 'vertical',
  stageBackgroundColor: '#fff0a6',
};

const gradients = {
  'qq-prism': 'linear-gradient(90deg, #10c8a0, #22c55e 48%, #f1c84b)',
  aurora: 'linear-gradient(90deg, #86efac, #22d3ee 48%, #60a5fa)',
  sunset: 'linear-gradient(90deg, #fbbf24, #fb7185 48%, #a78bfa)',
  classic: 'linear-gradient(90deg, #ffffff, #cffafe 56%, #e0e7ff)',
};

export function readLyricSettings() {
  try {
    const stored = JSON.parse(localStorage.getItem('rosettrism-lyric-settings') || 'null');
    return { ...defaultLyricSettings, ...(stored || {}) };
  } catch {
    return defaultLyricSettings;
  }
}

export function resolveLyricGradient(preset) {
  return gradients[preset] || gradients['qq-prism'];
}

export function normalizeLyricPayload(payload) {
  if (!payload || typeof payload !== 'object') {
    return { playable: false, lines: [], annotations: [], warnings: [] };
  }

  if (payload.raw || payload.format === 'raw') {
    return { playable: false, raw: payload.raw || '', lines: [], annotations: [], warnings: [] };
  }

  const unified = payload.unified || (isUnifiedLyric(payload.document) ? payload.document : null);
  if (unified) {
    return normalizeUnifiedLyric(unified, payload);
  }

  const document = payload.document || payload.result?.document;
  if (document?.lines) {
    return normalizeLyricDocument(document, payload);
  }

  return { playable: false, lines: [], annotations: [], warnings: [] };
}

function isUnifiedLyric(value) {
  return Boolean(value && typeof value === 'object' && (Array.isArray(value.inline_lines) || Array.isArray(value.tracks)));
}

export function normalizeUnifiedLyric(unified, context = {}) {
  const base = {
    kind: 'unified',
    source: context.source || 'aggregate',
    mode: unified.mode,
    title: unified.meta?.title || context.result?.title || '',
    artist: unified.meta?.artist || context.result?.artist || '',
    warnings: unified.warnings || [],
  };

  let lines = [];
  if (Array.isArray(unified.inline_lines) && unified.inline_lines.length > 0) {
    lines = unified.inline_lines.map((line, index) => normalizeLine(line, index, 'inline'));
  } else {
    const track = pickPrimaryTrack(unified.tracks || []);
    if (track?.document?.lines) {
      lines = track.document.lines.map((line, index) => normalizeLine(line, index, track.kind || 'track'));
      base.source = track.source || base.source;
      base.title = track.document.meta?.title || base.title;
      base.artist = track.document.meta?.artist || base.artist;
    }
  }

  lines = enrichLinesFromTracks(lines, unified.tracks || []);
  lines = deriveLineEndTimes(markLeadingMetadataLines(lines));
  const annotations = normalizeAnnotations(collectAnnotationSources(context, unified, null));
  lines = attachAnnotationsToLines(lines, annotations);
  return buildNormalizedLyric(base, lines, annotations);
}

export function normalizeLyricDocument(document, context = {}) {
  let lines = deriveLineEndTimes(markLeadingMetadataLines((document.lines || []).map((line, index) => normalizeLine(line, index, 'document'))));
  const annotations = normalizeAnnotations(collectAnnotationSources(context, null, document));
  lines = attachAnnotationsToLines(lines, annotations);
  return buildNormalizedLyric({
    kind: 'document',
    source: context.source || document.meta?.source || context.result?.source || '',
    title: document.meta?.title || context.result?.title || '',
    artist: document.meta?.artist || context.result?.artist || '',
    warnings: [],
  }, lines, annotations);
}

function collectAnnotationSources(context = {}, unified = null, document = null) {
  const selectedEntry = context.selectedEntry || {};
  const result = context.result || {};
  const roots = [context, result, selectedEntry, unified, document, context.extra, result.extra, selectedEntry.extra];
  const directSources = [];

  for (const root of roots) {
    collectAnnotationArrays(root, directSources);
  }

  if (Array.isArray(selectedEntry.extra?.aggregate_members)) {
    for (const member of selectedEntry.extra.aggregate_members) {
      collectAnnotationArrays(member, directSources);
      collectAnnotationArrays(member.extra, directSources);
    }
  }

  const seen = new Set();
  return directSources
    .flat()
    .filter((annotation) => {
      if (!annotation || typeof annotation !== 'object') {
        return false;
      }
      const key = `${annotation.annotation_type || annotation.annotationType || annotation.type || annotation.kind || ''}:${annotation.start_ms ?? annotation.startMs ?? annotation.time_ms ?? annotation.timeMs ?? ''}:${annotation.duration_ms ?? annotation.durationMs ?? annotation.duration ?? ''}:${annotation.text || annotation.label || ''}`;
      if (seen.has(key)) {
        return false;
      }
      seen.add(key);
      return true;
    });
}

function collectAnnotationArrays(value, sources, depth = 0, seen = new Set(), forced = false) {
  if (!value || typeof value !== 'object' || depth > 6 || seen.has(value)) {
    return;
  }
  seen.add(value);
  if (Array.isArray(value)) {
    const annotations = value.filter((item) => isAnnotationLike(item, forced));
    if (annotations.length) {
      sources.push(annotations);
    }
    for (const item of value) {
      collectAnnotationArrays(item, sources, depth + 1, seen, forced);
    }
    return;
  }
  for (const [key, child] of Object.entries(value)) {
    const annotationKey = /annotation|singing|assist|vocal/i.test(key);
    collectAnnotationArrays(child, sources, depth + 1, seen, forced || annotationKey);
  }
}

function isAnnotationLike(value, forced = false) {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const hasTime = Number.isFinite(Number(value.start_ms ?? value.startMs ?? value.time_ms ?? value.timeMs));
  const hasAnnotationType = Boolean(value.annotation_type || value.annotationType || value.annotation_kind || value.annotationKind);
  const explicitAnnotation = Object.keys(value).some((key) => /annotation|singing|assist|vocal/i.test(key));
  return hasTime && (forced || hasAnnotationType || explicitAnnotation);
}

function buildNormalizedLyric(base, lines, annotations) {
  const playableLines = lines.filter((line) => Number.isFinite(line.startMs));
  const lastLine = playableLines[playableLines.length - 1];
  const durationMs = Math.max(
    lastLine?.endMs || 0,
    ...annotations.map((annotation) => annotation.endMs || annotation.startMs || 0),
    1000,
  );

  return {
    ...base,
    playable: playableLines.length > 0,
    durationMs,
    lines: playableLines,
    annotations,
  };
}

function pickPrimaryTrack(tracks) {
  return tracks.find((track) => track.kind === 'original' && track.document?.lines?.length)
    || tracks.find((track) => track.document?.lines?.some((line) => Number.isFinite(line.start_ms)))
    || tracks.find((track) => track.document?.lines?.length)
    || null;
}

function enrichLinesFromTracks(lines, tracks) {
  const translationTracks = tracks.filter((track) => track.kind === 'translation' && track.document?.lines?.length);
  if (!translationTracks.length) {
    return lines;
  }
  return lines.map((line) => {
    if (line.translation || line.englishTranslation) {
      return line;
    }
    const match = translationTracks
      .map((track) => nearestTimedLine(track.document.lines || [], line.startMs))
      .find(Boolean);
    if (!match?.text) {
      return line;
    }
    return looksEnglish(match.text)
      ? { ...line, englishTranslation: match.text }
      : { ...line, translation: match.text };
  });
}

function nearestTimedLine(lines, startMs) {
  let bestLine = null;
  let bestDistance = Number.POSITIVE_INFINITY;
  for (const line of lines) {
    const lineStart = Number(line.start_ms ?? line.startMs ?? 0);
    const distance = Math.abs(lineStart - startMs);
    if (distance < bestDistance) {
      bestLine = line;
      bestDistance = distance;
    }
  }
  return bestDistance <= 1800 ? bestLine : null;
}

function normalizeLine(line, index, trackId) {
  const startMs = Number(line.start_ms ?? line.startMs ?? 0);
  const durationMs = optionalNumber(line.duration_ms ?? line.durationMs);
  const text = line.text || '';
  return {
    id: `${trackId}-${index}-${startMs}`,
    startMs,
    durationMs,
    endMs: durationMs ? startMs + durationMs : startMs,
    text,
    isMeta: isMetaLyricLine(text),
    translation: textValue(line.translation ?? line.translation_text ?? line.translationText ?? line.translated ?? line.trans ?? line.extra?.translation),
    englishTranslation: textValue(line.english_translation ?? line.englishTranslation ?? line.english ?? line.en_translation ?? line.enTranslation ?? line.extra?.englishTranslation ?? line.extra?.english),
    reading: textValue(line.reading),
    romanized: textValue(line.romanized),
    ruby: line.ruby || [],
    words: normalizeWords(line.words || [], startMs, index),
    annotations: [],
  };
}

function markLeadingMetadataLines(lines) {
  let metadataOpen = true;
  const hasLeadingCredits = lines.slice(1, 6).some((line) => isMetaLyricLine(line.text));
  return lines.map((line, index) => {
    const isLeadingTitle = index === 0 && (hasLeadingCredits || looksLikeLyricTitle(line.text));
    const isLeadingCredit = metadataOpen && isMetaLyricLine(line.text);
    const isMeta = metadataOpen && (isLeadingTitle || isLeadingCredit);
    if (!isMeta) {
      metadataOpen = false;
      return { ...line, isMeta: false };
    }
    return { ...line, isMeta: true, words: [] };
  });
}

function isMetaLyricLine(text) {
  const value = String(text || '').trim();
  if (!value) {
    return false;
  }
  return /^(作词|作曲|编曲|词|曲|制作人|制作|监制|原唱|翻唱|演唱|歌手|Lyricist|Lyrics|Composer|Music|Arranger|Producer|Vocal)\s*[:：]/i.test(value)
    || /^\[[a-z]{2,}\s*[:：].+\]$/i.test(value);
}

function looksLikeLyricTitle(text) {
  const value = String(text || '').trim();
  if (!value || isMetaLyricLine(value)) {
    return false;
  }
  if (value.length > 64 || /[。！？!?，,、；;]/.test(value)) {
    return false;
  }
  return /[-－—–]/.test(value) || /\s(?:DaveWang|Wang|王杰|歌手|演唱)/i.test(value);
}

function normalizeWords(words, lineStartMs, lineIndex) {
  return words
    .map((word, index) => {
      const offsetMs = Number(word.offset_ms ?? word.offsetMs ?? 0);
      const startMs = lineStartMs + offsetMs;
      const durationMs = optionalNumber(word.duration_ms ?? word.durationMs) || 0;
      return {
        id: `${lineIndex}-${index}-${startMs}`,
        startMs,
        durationMs,
        endMs: startMs + durationMs,
        text: word.text || '',
        annotations: [],
      };
    })
    .filter((word) => word.text);
}

export function deriveLineEndTimes(lines) {
  return lines.map((line, index) => {
    const next = lines[index + 1];
    const fallbackEnd = next?.startMs && next.startMs > line.startMs ? next.startMs : line.startMs + 4200;
    const endMs = line.durationMs ? line.startMs + line.durationMs : fallbackEnd;
    return { ...line, endMs };
  });
}

export function normalizeAnnotations(annotations) {
  return annotations
    .map((annotation, index) => {
      const type = normalizeAnnotationType(annotation.annotation_type || annotation.annotationType || annotation.annotation_kind || annotation.annotationKind || annotation.kind || annotation.type);
      const meta = annotationMeta(type);
      const startMs = Number(annotation.start_ms ?? annotation.startMs ?? annotation.time_ms ?? annotation.timeMs ?? 0);
      const durationMs = optionalNumber(annotation.duration_ms ?? annotation.durationMs ?? annotation.duration) || 800;
      return {
        id: `annotation-${index}-${startMs}`,
        type,
        startMs,
        durationMs,
        endMs: startMs + durationMs,
        text: annotation.text || '',
        ...meta,
      };
    })
    .filter((annotation) => Number.isFinite(annotation.startMs));
}

export function attachAnnotationsToLines(lines, annotations) {
  const nextLines = lines.map((line) => ({
    ...line,
    words: line.words.map((word) => ({ ...word, annotations: [] })),
    annotations: [],
  }));

  for (const annotation of annotations) {
    const lineIndex = findAnnotationLineIndex(nextLines, annotation);
    const line = nextLines[lineIndex];
    if (!line) {
      continue;
    }
    let matchedWord = false;
    const words = line.words.map((word) => {
      const timedMatch = annotation.startMs >= word.startMs - 420 && annotation.startMs <= Math.max(word.endMs, word.startMs + 1) + 420;
      const overlapMatch = annotation.endMs > word.startMs - 320 && annotation.startMs < Math.max(word.endMs, word.startMs + 1) + 320;
      const textMatch = annotation.text && word.text.includes(annotation.text);
      const matches = timedMatch || overlapMatch || textMatch;
      if (matches) {
        matchedWord = true;
      }
      return matches ? { ...word, annotations: [...word.annotations, annotation] } : word;
    });
    if (!matchedWord && words.length > 0) {
      const nearestIndex = findNearestWordIndex(words, annotation);
      words[nearestIndex] = { ...words[nearestIndex], annotations: [...words[nearestIndex].annotations, annotation] };
    }
    nextLines[lineIndex] = { ...line, words, annotations: [...line.annotations, annotation] };
  }

  return nextLines;
}

function findNearestWordIndex(words, annotation) {
  const midpoint = annotation.startMs + annotation.durationMs / 2;
  let bestIndex = 0;
  let bestDistance = Number.POSITIVE_INFINITY;
  for (let index = 0; index < words.length; index += 1) {
    const word = words[index];
    const center = word.startMs + (word.endMs - word.startMs) / 2;
    const distance = Math.abs(midpoint - center);
    if (distance < bestDistance) {
      bestIndex = index;
      bestDistance = distance;
    }
  }
  return bestIndex;
}

function findAnnotationLineIndex(lines, annotation) {
  const midpoint = annotation.startMs + annotation.durationMs / 2;
  let bestIndex = findActiveLineIndex(lines, annotation.startMs);
  let bestDistance = Number.POSITIVE_INFINITY;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const contains = annotation.startMs >= line.startMs - 600 && annotation.startMs <= line.endMs + 600;
    const center = line.startMs + (line.endMs - line.startMs) / 2;
    const distance = Math.abs(midpoint - center);
    if (contains && distance < bestDistance) {
      bestIndex = index;
      bestDistance = distance;
    }
  }
  return bestIndex;
}

function normalizeAnnotationType(type) {
  return String(type || 'unknown')
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/-/g, '_')
    .toLowerCase();
}

export function annotationMeta(type) {
  const map = {
    stress: { symbol: '`', labelKey: 'annotationStress', className: 'annotation-stress' },
    breath: { symbol: '^', labelKey: 'annotationBreath', className: 'annotation-breath' },
    long_tone: { symbol: '_', labelKey: 'annotationLongTone', className: 'annotation-long-tone' },
    portamento_up: { symbol: '↑', labelKey: 'annotationPortamentoUp', className: 'annotation-portamento-up' },
    portamento_down: { symbol: '↓', labelKey: 'annotationPortamentoDown', className: 'annotation-portamento-down' },
  };
  return map[type] || { symbol: '•', labelKey: 'annotations', className: 'annotation-unknown' };
}

export function findActiveLineIndex(lines, currentMs) {
  if (!lines.length) {
    return -1;
  }
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (currentMs >= line.startMs && currentMs < line.endMs) {
      return index;
    }
  }
  if (currentMs < lines[0].startMs) {
    return 0;
  }
  return lines.length - 1;
}

export function findVisibleLineIndex(lines, currentMs) {
  if (!lines.length) {
    return -1;
  }
  let active = 0;
  for (let index = 0; index < lines.length; index += 1) {
    if (currentMs >= lines[index].startMs) {
      active = index;
    }
  }
  return active;
}

export function findActiveWordIndex(words, currentMs) {
  if (!words.length) {
    return -1;
  }
  return words.findIndex((word) => currentMs >= word.startMs && currentMs < Math.max(word.endMs, word.startMs + 1));
}

export function formatPlaybackTime(ms) {
  const totalSeconds = Math.max(0, Math.floor((ms || 0) / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, '0');
  return `${minutes}:${seconds}`;
}

function textValue(value) {
  if (typeof value === 'string') {
    return value.trim();
  }
  if (value && typeof value === 'object') {
    return textValue(value.text ?? value.value ?? value.content);
  }
  return '';
}

function looksEnglish(value) {
  const text = String(value || '').trim();
  return /[A-Za-z]/.test(text) && !/[一-鿿]/.test(text);
}

function optionalNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : null;
}
