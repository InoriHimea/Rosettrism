export function formatArtistLabel(artist, alias) {
  const cleanArtist = String(artist || '').trim();
  const cleanAlias = String(alias || '').trim();
  if (!cleanArtist) {
    return cleanAlias;
  }
  return cleanAlias && cleanAlias !== cleanArtist ? `${cleanArtist}（${cleanAlias}）` : cleanArtist;
}

export function displayTitleIncludesArtist(title, artist) {
  const cleanTitle = normalizeDisplayText(title);
  const cleanArtist = normalizeDisplayText(artist);
  return Boolean(cleanArtist && cleanTitle.includes(cleanArtist));
}

function normalizeDisplayText(value) {
  return String(value || '')
    .replace(/\s+/g, '')
    .replace(/[()（）]/g, '')
    .toLowerCase();
}

export function visibleWordAnnotations(words, word, wordIndex) {
  const annotations = word.annotations || [];
  if (!annotations.length) {
    return annotations;
  }
  return annotations.filter((annotation) => {
    if (!isSingleWordAnnotation(annotation)) {
      return true;
    }
    return words.findIndex((item) => (item.annotations || []).some((candidate) => candidate.id === annotation.id)) === wordIndex;
  });
}

function isSingleWordAnnotation() {
  return true;
}

export function shouldReserveBreathGap(words, wordIndex, annotations) {
  const hasLeadingBreath = annotations.some((annotation) => annotation.type === 'breath' && annotationAnchorPercent(annotation) <= 12);
  if (!hasLeadingBreath) {
    return false;
  }
  return words.slice(0, wordIndex).some((word) => /\S/.test(String(word.text || '')));
}

export function translationModeLabel(mode, t) {
  if (mode === 'translation') {
    return t.lyricTranslationOnly || '译文';
  }
  if (mode === 'bilingual') {
    return t.lyricTranslationBilingual || '双语';
  }
  return t.lyricTranslationOff || '原文';
}

export function annotationLegendLabel(annotation, t) {
  const label = t[annotation.labelKey] || annotation.type;
  return `${annotationGlyphText(annotation.type)}=${label}`;
}

export function annotationGlyphText(type) {
  switch (type) {
    case 'stress':
      return '·';
    case 'breath':
      return 'V';
    case 'long_tone':
      return '_';
    default:
      return '';
  }
}

export function uniqueAnnotations(annotations) {
  const unique = new Map();
  for (const annotation of annotations) {
    const key = annotation.id || `${annotation.type}:${annotation.startMs}:${annotation.text}:${annotation.anchorKey || ''}`;
    if (!unique.has(key)) {
      unique.set(key, annotation);
    }
  }
  return [...unique.values()];
}

export function annotationLabelState(annotations) {
  const groups = new Map();
  annotations.forEach((annotation, index) => {
    if (annotation.suppressLabel) {
      return;
    }
    const key = annotationAnchorKey(annotation);
    const candidate = { annotation, index, priority: annotationLabelPriority(annotation.type) };
    const current = groups.get(key);
    if (!current || candidate.priority < current.priority || (candidate.priority === current.priority && index < current.index)) {
      groups.set(key, candidate);
    }
  });

  const labels = [...groups.values()].sort((left, right) => {
    const percent = annotationAnchorPercent(left.annotation) - annotationAnchorPercent(right.annotation);
    return Math.abs(percent) > 0.01 ? percent : left.index - right.index;
  });
  return {
    ids: new Set(labels.map((entry) => entry.annotation.id)),
    rows: new Map(labels.map((entry, row) => [entry.annotation.id, row])),
  };
}

export function annotationAnchorKey(annotation) {
  return annotation.anchorKey || `${Math.round(annotationAnchorPercent(annotation) / 6)}`;
}

export function annotationAnchorPercent(annotation) {
  const value = Number(annotation.anchorPercent);
  return Number.isFinite(value) ? Math.max(0, Math.min(100, value)) : 50;
}

export function annotationLabelPriority(type) {
  switch (type) {
    case 'breath':
      return 0;
    case 'stress':
      return 1;
    case 'long_tone':
      return 2;
    default:
      return 3;
  }
}

export function lyricProgressStyle(progress, exact = false) {
  const safeProgress = clampProgress(progress);
  const fillEnd = exact || safeProgress === 0 || safeProgress === 1
    ? safeProgress * 100
    : Math.min(100, safeProgress * 100 + 2.4);
  return {
    '--lyric-progress': String(safeProgress),
    '--lyric-fill-end': `${fillEnd}%`,
  };
}

export function lyricLineProgress(line, currentMs) {
  const start = line.startMs;
  const end = Math.max(line.endMs || start + 1, start + 1);
  return clampProgress((currentMs - start) / (end - start));
}

export function wordProgress(word, currentMs) {
  const start = word.startMs;
  const end = Math.max(word.endMs || start + 1, start + 1);
  return clampProgress((currentMs - start) / (end - start));
}

export function clampProgress(value) {
  return Math.max(0, Math.min(1, value));
}

export function lineClassName(line, currentMs, active, bodyIndex, focusBodyIndex) {
  const state = active ? 'lyric-line-active' : line.endMs <= currentMs ? 'lyric-line-past' : 'lyric-line-future';
  const distance = focusBodyIndex < 0 ? 4 : Math.min(4, Math.abs(bodyIndex - focusBodyIndex));
  return `lyric-line ${state} lyric-line-distance-${distance}${line?.isMeta ? ' lyric-line-meta' : ''}`;
}

export function findActiveTimedLineIndex(lines, currentMs) {
  return lines.findIndex((line) => currentMs >= line.startMs && currentMs < line.endMs);
}

export function buildIntroMetaLines(lines, firstBodyStartMs = 0) {
  const unique = [];
  const seen = new Set();
  const titleText = firstMetaTitleText(lines);
  for (const line of lines) {
    const text = String(line.text || '').trim();
    const comparable = normalizeMetaText(text);
    if (!text || seen.has(comparable)) {
      continue;
    }
    if (titleText && comparable !== normalizeMetaText(titleText) && looksLikeTitleDuplicate(text, titleText)) {
      continue;
    }
    seen.add(comparable);
    unique.push({
      ...line,
      id: `intro-${line.id}`,
      isMeta: true,
      words: [],
      annotations: [],
    });
  }

  const durationMs = introMetaDurationMs(unique.length, firstBodyStartMs);
  return unique.map((line, index) => ({
    ...line,
    startMs: index * durationMs,
    durationMs,
    endMs: (index + 1) * durationMs,
  }));
}

export function introMetaDurationMs(count, firstBodyStartMs) {
  if (!count) {
    return 1200;
  }
  if (!Number.isFinite(firstBodyStartMs) || firstBodyStartMs <= 0) {
    return 1200;
  }
  const availableMs = Math.max(720, firstBodyStartMs - 1200);
  const perLineMs = Math.floor(availableMs / count);
  return Math.max(perLineMs < 1000 ? 720 : 1000, Math.min(2000, perLineMs));
}

function firstMetaTitleText(lines) {
  return String(lines.find((line) => String(line.id || '').startsWith('meta-title-'))?.text || '').trim();
}

export function normalizeMetaText(text) {
  return String(text || '')
    .replace(/（[^）]*）|\([^)]*\)/g, '')
    .replace(/([㐀-鿿])([A-Za-z][\w .'-]*)$/u, '$1')
    .replace(/[\s—–－-]+/g, '')
    .trim()
    .toLowerCase();
}

export function looksLikeTitleDuplicate(text, titleText) {
  const normalized = normalizeMetaText(text);
  const title = normalizeMetaText(titleText);
  return Boolean(title && (normalized === title || title.includes(normalized) || normalized.includes(title)));
}

export function karaokeLaneItems(bodyLines, activeBodyLine, focusBodyIndex, showCountdown, countdown) {
  if (!bodyLines.length) {
    return [];
  }
  if (showCountdown) {
    const targetIndex = bodyLines.findIndex((line) => line.id === countdown.targetLineId);
    const targetLine = targetIndex >= 0 ? bodyLines[targetIndex] : null;
    const targetLane = targetLine
      ? karaokeLineLaneItem(targetLine, targetIndex)
      : karaokeLineLaneItem(bodyLines[Math.max(0, Math.min(bodyLines.length - 1, focusBodyIndex))], Math.max(0, focusBodyIndex));
    return [
      {
        kind: 'countdown',
        key: countdown.targetLineId ? `countdown-${countdown.targetLineId}` : 'countdown-gap',
        targetLine,
        bodyIndex: targetIndex,
        laneClass: targetLane.laneClass,
        lanePositionClass: targetLane.lanePositionClass,
      },
    ];
  }

  const primaryIndex = Math.max(0, Math.min(bodyLines.length - 1, focusBodyIndex));
  const secondaryIndex = primaryIndex + 1 < bodyLines.length ? primaryIndex + 1 : primaryIndex - 1;
  const indexes = [primaryIndex, secondaryIndex].filter((index, position, list) => index >= 0 && index < bodyLines.length && list.indexOf(index) === position);
  return indexes
    .map((index) => karaokeLineLaneItem(bodyLines[index], index))
    .sort((left, right) => laneSortIndex(left.lanePositionClass) - laneSortIndex(right.lanePositionClass));
}

export function karaokeLineLaneItem(line, bodyIndex) {
  const topLane = bodyIndex % 2 === 0;
  return {
    kind: 'line',
    line,
    bodyIndex,
    laneClass: topLane ? 'lyric-karaoke-line-left' : 'lyric-karaoke-line-right',
    lanePositionClass: topLane ? 'lyric-karaoke-lane-top' : 'lyric-karaoke-lane-bottom',
  };
}

export function karaokePlaceholderLanes(items) {
  const occupied = new Set(items.map((item) => item.lanePositionClass).filter(Boolean));
  return ['lyric-karaoke-lane-top', 'lyric-karaoke-lane-bottom'].filter((laneClass) => !occupied.has(laneClass));
}

export function laneSortIndex(lanePositionClass) {
  return lanePositionClass === 'lyric-karaoke-lane-top' ? 0 : 1;
}

export function lyricCountdown(lines, currentMs, { introMetaEndMs = 0 } = {}) {
  const exitingLine = lines.find((line, index) => {
    const kind = countdownSeparatorKind(lines, index, introMetaEndMs);
    return kind && currentMs >= line.startMs && currentMs < line.startMs + 420;
  });
  if (exitingLine) {
    const exitingIndex = lines.findIndex((line) => line.id === exitingLine.id);
    return {
      count: 1,
      flashing: false,
      exiting: true,
      remainingMs: Math.max(0, exitingLine.startMs + 420 - currentMs),
      targetLineId: exitingLine.id,
      kind: countdownSeparatorKind(lines, exitingIndex, introMetaEndMs),
      visible: true,
    };
  }

  const nextIndex = lines.findIndex((line) => line.startMs > currentMs);
  const nextLine = nextIndex >= 0 ? lines[nextIndex] : null;
  if (!nextLine) {
    return { count: 0, flashing: false, exiting: false, remainingMs: 0, targetLineId: null, kind: 'gap', visible: false };
  }

  const remainingMs = nextLine.startMs - currentMs;
  const kind = countdownSeparatorKind(lines, nextIndex, introMetaEndMs);
  const introBlockedByMeta = kind === 'intro' && introMetaEndMs > 0 && currentMs < introMetaEndMs;
  const interludeBlockedByLyric = kind === 'interlude' && currentMs < previousBodyEndMs(lines, nextIndex);
  if (!kind || introBlockedByMeta || interludeBlockedByLyric) {
    return { count: 0, flashing: false, exiting: false, remainingMs, targetLineId: nextLine.id, kind: kind || 'gap', visible: false };
  }
  if (remainingMs <= 1200) {
    return { count: 1, flashing: false, exiting: false, remainingMs, targetLineId: nextLine.id, kind, visible: true };
  }
  if (remainingMs <= 2200) {
    return { count: 2, flashing: false, exiting: false, remainingMs, targetLineId: nextLine.id, kind, visible: true };
  }
  return { count: 3, flashing: remainingMs > 3200, exiting: false, remainingMs, targetLineId: nextLine.id, kind, visible: true };
}

export function countdownSeparatorKind(lines, index, introMetaEndMs = 0) {
  if (index < 0 || !lines[index]) {
    return null;
  }
  if (index === 0) {
    const introGapMs = lines[index].startMs - Math.max(0, introMetaEndMs);
    if (introMetaEndMs > 0) {
      return introGapMs >= 900 ? 'intro' : null;
    }
    return lines[index].startMs >= 3200 ? 'intro' : null;
  }
  const previousEndMs = previousBodyEndMs(lines, index);
  return lines[index].startMs - previousEndMs >= 5200 ? 'interlude' : null;
}

export function previousBodyEndMs(lines, index) {
  const previous = lines[index - 1];
  if (!previous) {
    return 0;
  }
  return Number.isFinite(previous.endMs) ? previous.endMs : previous.startMs;
}
