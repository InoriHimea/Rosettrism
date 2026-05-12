export const defaultLyricSettings = {
  colorMode: 'gradient',
  colorPreset: 'qq-prism',
  solidColor: '#f8fbff',
};

const gradients = {
  'qq-prism': 'linear-gradient(90deg, #67e8f9, #c084fc 48%, #f9a8d4)',
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

  lines = deriveLineEndTimes(lines);
  const annotations = normalizeAnnotations(unified.annotations || []);
  lines = attachAnnotationsToLines(lines, annotations);
  return buildNormalizedLyric(base, lines, annotations);
}

export function normalizeLyricDocument(document, context = {}) {
  let lines = deriveLineEndTimes((document.lines || []).map((line, index) => normalizeLine(line, index, 'document')));
  const annotations = normalizeAnnotations(context.annotations || document.annotations || context.result?.annotations || []);
  lines = attachAnnotationsToLines(lines, annotations);
  return buildNormalizedLyric({
    kind: 'document',
    source: context.source || document.meta?.source || context.result?.source || '',
    title: document.meta?.title || context.result?.title || '',
    artist: document.meta?.artist || context.result?.artist || '',
    warnings: [],
  }, lines, annotations);
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

function normalizeLine(line, index, trackId) {
  const startMs = Number(line.start_ms ?? line.startMs ?? 0);
  const durationMs = optionalNumber(line.duration_ms ?? line.durationMs);
  return {
    id: `${trackId}-${index}-${startMs}`,
    startMs,
    durationMs,
    endMs: durationMs ? startMs + durationMs : startMs,
    text: line.text || '',
    translation: line.translation || '',
    reading: line.reading || '',
    romanized: line.romanized || '',
    ruby: line.ruby || [],
    words: normalizeWords(line.words || [], startMs, index),
    annotations: [],
  };
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
      const type = normalizeAnnotationType(annotation.annotation_type || annotation.annotationType || annotation.type);
      const meta = annotationMeta(type);
      const startMs = Number(annotation.start_ms ?? annotation.startMs ?? 0);
      const durationMs = optionalNumber(annotation.duration_ms ?? annotation.durationMs) || 800;
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
    const words = line.words.map((word) => {
      const timedMatch = annotation.startMs >= word.startMs - 120 && annotation.startMs < Math.max(word.endMs, word.startMs + 1) + 120;
      const textMatch = annotation.text && word.text.includes(annotation.text);
      return timedMatch || textMatch ? { ...word, annotations: [...word.annotations, annotation] } : word;
    });
    nextLines[lineIndex] = { ...line, words, annotations: [...line.annotations, annotation] };
  }

  return nextLines;
}

function findAnnotationLineIndex(lines, annotation) {
  const midpoint = annotation.startMs + annotation.durationMs / 2;
  let bestIndex = findActiveLineIndex(lines, annotation.startMs);
  let bestDistance = Number.POSITIVE_INFINITY;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const contains = annotation.startMs >= line.startMs - 180 && annotation.startMs <= line.endMs + 180;
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

function optionalNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : null;
}
