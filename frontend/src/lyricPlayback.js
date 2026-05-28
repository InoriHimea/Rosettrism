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

export function formatSourceName(raw) {
  const value = textValue(raw);
  if (!value) {
    return '';
  }
  return value
    .replace(/QQ\s*音乐/gi, 'Tencent')
    .replace(/(^|[^A-Za-z0-9])qq(?:[-_\s]?music)?(?=$|[^A-Za-z0-9])/gi, '$1Tencent');
}

export function formatInputFormat(raw) {
  const value = textValue(raw);
  return value ? value.replace(/_/g, '-').toUpperCase() : '';
}

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

  return normalizeEntryPreview(payload.selectedEntry || payload.result || payload);
}

function isUnifiedLyric(value) {
  return Boolean(value && typeof value === 'object' && (Array.isArray(value.inline_lines) || Array.isArray(value.tracks)));
}

function normalizeEntryPreview(entry) {
  if (!entry || typeof entry !== 'object') {
    return { playable: false, lines: [], annotations: [], warnings: [] };
  }
  const unified = entry.unified || entry.extra?.unified || (isUnifiedLyric(entry.document) ? entry.document : null);
  if (unified) {
    return normalizeUnifiedLyric(unified, { selectedEntry: entry, result: entry });
  }
  const document = entry.document || entry.extra?.document;
  if (document?.lines) {
    return normalizeLyricDocument(document, { selectedEntry: entry, result: entry });
  }
  return { playable: false, lines: [], annotations: normalizeAnnotations(collectAnnotationSources({ selectedEntry: entry, result: entry })), warnings: [] };
}

export function normalizeUnifiedLyric(unified, context = {}) {
  const base = {
    kind: 'unified',
    source: context.source || unified.meta?.source || context.result?.source || context.selectedEntry?.source || 'aggregate',
    inputFormat: pickInputFormat(context, unified, context.result, context.selectedEntry),
    mode: unified.mode,
    title: unified.meta?.title || context.result?.title || context.selectedEntry?.title || '',
    artist: unified.meta?.artist || context.result?.artist || context.selectedEntry?.artist || '',
    artistAlias: pickArtistAlias(unified.meta, context.result, context.selectedEntry),
    warnings: unified.warnings || [],
  };

  let lines = [];
  if (Array.isArray(unified.inline_lines) && unified.inline_lines.length > 0) {
    lines = unified.inline_lines.map((line, index) => normalizeLine(line, index, 'inline'));
  } else {
    const track = pickPrimaryTrack(unified.tracks || []);
    if (track?.document?.lines) {
      lines = track.document.lines.map((line, index) => normalizeLine(line, index, track.kind || 'track'));
      lines = mergeInlineLines(lines, unified.inline_lines || []);
      base.source = track.source || base.source;
      base.inputFormat = pickInputFormat(context, track, track.document, track.document.meta, unified, context.result, context.selectedEntry) || base.inputFormat;
      base.title = track.document.meta?.title || base.title;
      base.artist = track.document.meta?.artist || base.artist;
      base.artistAlias = pickArtistAlias(track.document.meta, unified.meta, context.result, context.selectedEntry);
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
    source: context.source || document.meta?.source || context.result?.source || context.selectedEntry?.source || '',
    inputFormat: pickInputFormat(context, document, document.meta, context.result, context.selectedEntry),
    title: document.meta?.title || context.result?.title || context.selectedEntry?.title || '',
    artist: document.meta?.artist || context.result?.artist || context.selectedEntry?.artist || '',
    artistAlias: pickArtistAlias(document.meta, context.result, context.selectedEntry),
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
  const titleParts = inferTitleParts(base, playableLines[0]);
  const displayTitle = titleParts.displayTitle || formatDisplayTitle(titleParts.title, titleParts.artist, titleParts.artistAlias);
  const lastLine = playableLines[playableLines.length - 1];
  const durationMs = Math.max(
    lastLine?.endMs || 0,
    ...annotations.map((annotation) => annotation.endMs || annotation.startMs || 0),
    1000,
  );

  return {
    ...base,
    playable: playableLines.length > 0,
    source: formatSourceName(base.source),
    inputFormat: formatInputFormat(base.inputFormat || inferInputFormatFromLines(playableLines)),
    title: titleParts.title,
    artist: titleParts.artist,
    artistAlias: titleParts.artistAlias,
    displayTitle,
    durationMs,
    lines: ensureLeadingTitleLine(playableLines),
    annotations,
  };
}

function inferTitleParts(base, firstLine) {
  const lineDisplayTitle = lyricDisplayTitleText(firstLine);
  const baseDisplayTitle = lyricDisplayTitleText(base.title);
  const lineInferred = splitInlineArtistAlias(lineDisplayTitle);
  const baseInferred = splitInlineArtistAlias(base.title);
  const inferred = lineInferred.title ? lineInferred : baseInferred;
  const split = splitArtistAlias(base.artist || inferred.artist);
  const artist = inferred.artist || split.artist || base.artist;
  const artistAlias = normalizeArtistAlias(inferred.artistAlias || base.artistAlias || split.artistAlias);
  return {
    title: inferred.title || base.title,
    artist,
    artistAlias,
    displayTitle: lineDisplayTitle || baseDisplayTitle,
  };
}

function lyricDisplayTitleText(value) {
  const text = textValue(value?.text ?? value);
  if (!text || (value && typeof value === 'object' && value.isMeta === false)) {
    return '';
  }
  return looksLikeLyricTitle(text) ? text : '';
}

function splitInlineArtistAlias(value) {
  const text = textValue(value);
  let match = text.match(/^(.+?)\s*[-—–－]\s*([^\x00-\x7F\s()（）-]+)(?:\s*[（(]([^）)]*)[）)]|\s+([A-Za-z][\w .'-]*))?$/);
  if (!match) {
    match = text.match(/^(.+?)\s+([^\x00-\x7F\s()（）-]+)\s*[（(]([^）)]*)[）)]$/);
  }
  if (!match) {
    return {};
  }
  return {
    title: match[1].trim(),
    artist: match[2].trim(),
    artistAlias: normalizeArtistAlias(match[3] || match[4] || ''),
  };
}

function splitArtistAlias(value) {
  const text = textValue(value);
  const match = text.match(/^([^\x00-\x7F]+?)([A-Za-z][\w .'-]*)$/);
  if (!match) {
    return { artist: text, artistAlias: '' };
  }
  return {
    artist: match[1].trim(),
    artistAlias: normalizeArtistAlias(match[2]),
  };
}

function normalizeArtistAlias(value) {
  return textValue(value).replace(/\b[a-z]/g, (char) => char.toUpperCase());
}

function formatDisplayTitle(title, artist, artistAlias) {
  const cleanTitle = textValue(title);
  const cleanArtist = textValue(artist);
  const cleanAlias = textValue(artistAlias);
  const displayAlias = cleanAlias && cleanAlias !== cleanArtist ? cleanAlias : '';
  if (!cleanTitle) {
    return '';
  }
  if (!cleanArtist) {
    return cleanTitle;
  }
  return `${cleanTitle} - ${cleanArtist}${displayAlias ? `（${displayAlias}）` : ''}`;
}

function ensureLeadingTitleLine(lines) {
  return lines;
}

function pickArtistAlias(...sources) {
  for (const source of sources) {
    const alias = firstText(
      source?.artist_alias,
      source?.artistAlias,
      source?.artist_en,
      source?.artistEn,
      source?.english_artist,
      source?.englishArtist,
      source?.singer_alias,
      source?.singerAlias,
      source?.subtitle,
      source?.sub_title,
      source?.subTitle,
      source?.trans_name,
      source?.transName,
      source?.singer_trans_name,
      source?.singerTransName,
      source?.extra?.artist_alias,
      source?.extra?.artistAlias,
      source?.extra?.artist_en,
      source?.extra?.artistEn,
      source?.extra?.english_artist,
      source?.extra?.englishArtist,
      source?.extra?.singer_alias,
      source?.extra?.singerAlias,
      source?.extra?.subtitle,
      source?.extra?.sub_title,
      source?.extra?.subTitle,
      source?.extra?.trans_name,
      source?.extra?.transName,
      source?.extra?.singer_trans_name,
      source?.extra?.singerTransName,
    );
    if (alias) {
      return alias;
    }
  }
  return '';
}

function pickInputFormat(...sources) {
  return firstText(...sources.flatMap((source) => [
    source?.input_format,
    source?.inputFormat,
    source?.lyric_input_format,
    source?.lyricInputFormat,
    source?.extra?.input_format,
    source?.extra?.inputFormat,
    source?.extra?.lyric_input_format,
    source?.extra?.lyricInputFormat,
  ]));
}

function inferInputFormatFromLines(lines) {
  return lines.some((line) => line.words?.length) ? 'qrc' : '';
}

function firstText(...values) {
  for (const value of values) {
    if (Array.isArray(value)) {
      const text = value.map((item) => textValue(item)).find(Boolean);
      if (text) {
        return text;
      }
      continue;
    }
    const text = textValue(value);
    if (text) {
      return text;
    }
  }
  return '';
}

function pickPrimaryTrack(tracks) {
  return tracks.find((track) => track.kind === 'original' && track.document?.lines?.length)
    || tracks.find((track) => track.document?.lines?.some((line) => Number.isFinite(line.start_ms)))
    || tracks.find((track) => track.document?.lines?.length)
    || null;
}

function mergeInlineLines(lines, inlineLines) {
  if (!Array.isArray(inlineLines) || !inlineLines.length) {
    return lines;
  }
  return lines.map((line) => {
    const match = nearestTimedLine(inlineLines, line.startMs);
    if (!match) {
      return line;
    }
    return {
      ...line,
      translation: line.translation || textValue(match.translation ?? match.translation_text ?? match.translationText ?? match.translated ?? match.trans ?? match.extra?.translation),
      englishTranslation: line.englishTranslation || textValue(match.english_translation ?? match.englishTranslation ?? match.english ?? match.en_translation ?? match.enTranslation ?? match.extra?.englishTranslation ?? match.extra?.english),
      reading: line.reading || pickLineReading(match),
      romanized: line.romanized || pickLineRomanized(match),
    };
  });
}

function enrichLinesFromTracks(lines, tracks) {
  const timedTracks = (tracks || []).filter((track) => track.document?.lines?.length);
  if (!timedTracks.length) {
    return lines;
  }
  const translationTracks = timedTracks.filter((track) => isTranslationTrack(track));
  const readingTracks = timedTracks.filter((track) => isReadingTrack(track));
  const romanizedTracks = timedTracks.filter((track) => isRomanizedTrack(track));
  return lines.map((line) => {
    let nextLine = line;
    if (!nextLine.translation && !nextLine.englishTranslation) {
      const match = nearestTrackLine(translationTracks, nextLine.startMs);
      const text = textValue(match?.text);
      if (text) {
        nextLine = looksEnglish(text)
          ? { ...nextLine, englishTranslation: text }
          : { ...nextLine, translation: text };
      }
    }
    if (!nextLine.reading) {
      const match = nearestTrackLine(readingTracks, nextLine.startMs);
      const reading = pickLineReading(match) || textValue(match?.text);
      if (reading) {
        nextLine = { ...nextLine, reading };
      }
    }
    if (!nextLine.romanized) {
      const match = nearestTrackLine(romanizedTracks, nextLine.startMs);
      const romanized = pickLineRomanized(match) || textValue(match?.text);
      if (romanized) {
        nextLine = { ...nextLine, romanized };
      }
    }
    return nextLine;
  });
}

function nearestTrackLine(tracks, startMs) {
  return tracks
    .map((track) => nearestTimedLine(track.document.lines || [], startMs))
    .find(Boolean);
}

function isTranslationTrack(track) {
  const kind = normalizeTrackKind(track?.kind || track?.type || track?.name || track?.label);
  return /trans|translation|translated|english|en/.test(kind);
}

function isReadingTrack(track) {
  const kind = normalizeTrackKind(track?.kind || track?.type || track?.name || track?.label || track?.source);
  return /reading|ruby|kana|furigana|phonetic|phoneme|pronunciation|pronounce|jyutping|cantonese|yue|pinyin|roma|roman|transliteration|sound|syllable/.test(kind);
}

function isRomanizedTrack(track) {
  const kind = normalizeTrackKind(track?.kind || track?.type || track?.name || track?.label || track?.source);
  return /roman|roma|romaji|latin|transliteration|jyutping|cantonese|yue|phonetic|phoneme|pronunciation|pronounce|pinyin|sound|syllable/.test(kind);
}

function normalizeTrackKind(value) {
  return String(value || '').replace(/([a-z0-9])([A-Z])/g, '$1_$2').replace(/[-\s]+/g, '_').toLowerCase();
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
  const words = normalizeWords(line.words || line.chars || line.characters || line.extra?.words || [], startMs, index);
  return {
    id: `${trackId}-${index}-${startMs}`,
    startMs,
    durationMs,
    endMs: durationMs ? startMs + durationMs : startMs,
    text,
    isMeta: words.length ? false : isMetaLyricLine(text),
    translation: textValue(line.translation ?? line.translation_text ?? line.translationText ?? line.translated ?? line.trans ?? line.extra?.translation),
    englishTranslation: textValue(line.english_translation ?? line.englishTranslation ?? line.english ?? line.en_translation ?? line.enTranslation ?? line.extra?.englishTranslation ?? line.extra?.english),
    reading: pickLineReading(line),
    romanized: pickLineRomanized(line) || pickLineReading(line),
    ruby: normalizeRubySpans(line.ruby || line.furigana_spans || line.furiganaSpans || line.extra?.ruby || []),
    words,
    annotations: [],
  };
}

function markLeadingMetadataLines(lines) {
  let metadataOpen = true;
  const hasLeadingCredits = lines.slice(1, 6).some((line) => !line.words?.length && isMetaLyricLine(line.text));
  return lines.map((line, index) => {
    if (line.words?.length) {
      metadataOpen = false;
      return { ...line, isMeta: false };
    }
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
      const absoluteStartMs = optionalNumber(word.start_ms ?? word.startMs ?? word.time_ms ?? word.timeMs);
      const startMs = absoluteStartMs ?? lineStartMs + offsetMs;
      const durationMs = optionalNumber(word.duration_ms ?? word.durationMs ?? word.duration) || 0;
      return {
        id: `${lineIndex}-${index}-${startMs}`,
        startMs,
        durationMs,
        endMs: startMs + durationMs,
        text: textValue(word.text ?? word.value ?? word.char ?? word.character ?? word.content),
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
    const words = line.words.map((word) => ({ ...word }));
    if (words.length > 0) {
      const anchorIndex = findAnnotationAnchorWordIndex(words, annotation);
      words[anchorIndex] = { ...words[anchorIndex], annotations: [...words[anchorIndex].annotations, annotation] };
    }
    nextLines[lineIndex] = { ...line, words, annotations: [...line.annotations, annotation] };
  }

  return nextLines;
}

function findAnnotationAnchorWordIndex(words, annotation) {
  const targetText = textValue(annotation.text).trim();
  let bestIndex = 0;
  let bestDistance = Number.POSITIVE_INFINITY;

  if (targetText) {
    for (let index = 0; index < words.length; index += 1) {
      const word = words[index];
      if (word.text.trim() !== targetText) {
        continue;
      }
      const distance = Math.abs(annotation.startMs - word.startMs);
      if (distance < bestDistance) {
        bestIndex = index;
        bestDistance = distance;
      }
    }
    if (bestDistance < Number.POSITIVE_INFINITY) {
      return bestIndex;
    }
  }

  for (let index = 0; index < words.length; index += 1) {
    const word = words[index];
    const startDistance = Math.abs(annotation.startMs - word.startMs);
    const center = word.startMs + (word.endMs - word.startMs) / 2;
    const centerDistance = Math.abs(annotation.startMs - center);
    const distance = Math.min(startDistance, centerDistance);
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
    stress: { symbol: '·', labelKey: 'annotationStress', className: 'annotation-stress' },
    breath: { symbol: 'V', labelKey: 'annotationBreath', className: 'annotation-breath' },
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

function normalizeRubySpans(value) {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((span) => {
      const startChar = Number(span.start_char ?? span.startChar ?? span.start ?? span.from);
      const endChar = Number(span.end_char ?? span.endChar ?? span.end ?? span.to);
      const text = textValue(span.text ?? span.base ?? span.value);
      const reading = textValue(span.reading ?? span.rt ?? span.ruby ?? span.furigana ?? span.kana);
      if (!Number.isFinite(startChar) || !Number.isFinite(endChar) || endChar <= startChar || !reading) {
        return null;
      }
      return { startChar, endChar, text, reading };
    })
    .filter(Boolean)
    .sort((left, right) => left.startChar - right.startChar || left.endChar - right.endChar);
}

function pickLineReading(line) {
  return firstText(
    line?.reading,
    line?.readings,
    line?.ruby,
    line?.furigana,
    line?.kana,
    line?.pronunciation,
    line?.phonetic,
    line?.phonetics,
    line?.pinyin,
    line?.jyutping,
    line?.cantonese,
    line?.cantonese_romanization,
    line?.cantoneseRomanization,
    line?.yue,
    line?.yue_romanization,
    line?.yueRomanization,
    line?.phoneme,
    line?.phonemes,
    line?.pronounce,
    line?.pronounces,
    line?.extra?.reading,
    line?.extra?.readings,
    line?.extra?.ruby,
    line?.extra?.furigana,
    line?.extra?.kana,
    line?.extra?.pronunciation,
    line?.extra?.phonetic,
    line?.extra?.phonetics,
    line?.extra?.pinyin,
    line?.extra?.jyutping,
    line?.extra?.cantonese,
    line?.extra?.cantonese_romanization,
    line?.extra?.cantoneseRomanization,
    line?.extra?.yue,
    line?.extra?.yue_romanization,
    line?.extra?.yueRomanization,
    line?.extra?.phoneme,
    line?.extra?.phonemes,
    line?.extra?.pronounce,
    line?.extra?.pronounces,
  );
}

function pickLineRomanized(line) {
  return firstText(
    line?.romanized,
    line?.romanised,
    line?.romaji,
    line?.romanization,
    line?.romanisation,
    line?.romaji_text,
    line?.romajiText,
    line?.transliteration,
    line?.transliteration_text,
    line?.transliterationText,
    line?.latin,
    line?.latin_text,
    line?.latinText,
    line?.jyutping,
    line?.cantonese_romanization,
    line?.cantoneseRomanization,
    line?.extra?.romanized,
    line?.extra?.romanised,
    line?.extra?.romaji,
    line?.extra?.romanization,
    line?.extra?.romanisation,
    line?.extra?.romaji_text,
    line?.extra?.romajiText,
    line?.extra?.transliteration,
    line?.extra?.transliteration_text,
    line?.extra?.transliterationText,
    line?.extra?.latin,
    line?.extra?.latin_text,
    line?.extra?.latinText,
    line?.extra?.jyutping,
    line?.extra?.cantonese_romanization,
    line?.extra?.cantoneseRomanization,
  );
}

function textValue(value) {
  if (typeof value === 'string') {
    return value.trim();
  }
  if (Array.isArray(value)) {
    return value.map((item) => textValue(item)).filter(Boolean).join(' ');
  }
  if (value && typeof value === 'object') {
    return textValue(value.text ?? value.value ?? value.content ?? value.lyric ?? value.line);
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
