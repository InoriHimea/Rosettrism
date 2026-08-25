const severityRank = {
  info: 0,
  warning: 1,
  error: 2,
};

export function assessLyricQuality({
  lines = [],
  annotations = [],
  raw = '',
} = {}) {
  const diagnostics = [];
  const timedLines = lines.filter(hasExplicitLineTiming);
  const bodyLines = timedLines.filter((line) => !line.isMeta);
  const wordTimedLines = bodyLines.filter(hasUsableWordTiming);
  const metrics = {
    lineCount: lines.length,
    timedLineCount: timedLines.length,
    bodyLineCount: bodyLines.length,
    wordTimedLineCount: wordTimedLines.length,
    wordCount: bodyLines.reduce(
      (total, line) => total + (line.words?.length || 0),
      0,
    ),
    annotationCount: annotations.length,
  };

  if (!lines.length) {
    addDiagnostic(
      diagnostics,
      raw ? 'UNSYNCED_RAW_TEXT' : 'NO_LYRIC_LINES',
      'error',
    );
  }

  inspectLines(lines, diagnostics);
  inspectAnnotations(timedLines, annotations, diagnostics);

  const hasLineTiming = bodyLines.length > 0;
  const hasWordTiming = wordTimedLines.length > 0;
  const blocking = diagnostics.some((item) => item.severity === 'error');
  let timingLevel = 'unsynced';
  if (hasLineTiming) {
    timingLevel = hasWordTiming ? 'word_timed' : 'line_timed';
  }
  if (blocking && lines.length) {
    timingLevel = 'invalid';
  }

  if (hasLineTiming && !hasWordTiming) {
    addDiagnostic(diagnostics, 'WORD_TIMING_UNAVAILABLE', 'info');
  }

  const capabilities = {
    synced: hasLineTiming && !blocking,
    lineTiming: hasLineTiming,
    wordTiming: hasWordTiming,
    translation: bodyLines.some(hasTranslation),
    reading: bodyLines.some(hasReading),
    ruby: bodyLines.some((line) => line.ruby?.length > 0),
    annotations: annotations.length > 0,
  };
  const playable = capabilities.synced;
  const severity = diagnostics.reduce(
    (current, item) => (
      severityRank[item.severity] > severityRank[current]
        ? item.severity
        : current
    ),
    'info',
  );

  return {
    version: 1,
    timingLevel,
    playable,
    severity,
    capabilities,
    metrics,
    diagnostics: dedupeDiagnostics(diagnostics),
    degradationReasons: diagnostics
      .filter((item) => item.severity !== 'info')
      .map((item) => item.code),
  };
}

function inspectLines(lines, diagnostics) {
  let previousStart = null;
  let previousEnd = null;

  lines.forEach((line, lineIndex) => {
    if (!String(line.text || '').trim()) {
      addDiagnostic(diagnostics, 'EMPTY_LINE_TEXT', 'warning', { lineIndex });
    }
    if (!hasExplicitLineTiming(line)) {
      addDiagnostic(diagnostics, 'LINE_TIMING_MISSING', 'error', { lineIndex });
      return;
    }

    const startMs = Number(line.startMs);
    const endMs = lineEndMs(line);
    if (previousStart !== null && startMs < previousStart) {
      addDiagnostic(diagnostics, 'LINE_TIMING_OUT_OF_ORDER', 'error', {
        lineIndex,
        startMs,
        previousStartMs: previousStart,
      });
    }
    if (previousEnd !== null && startMs < previousEnd && !line.isMeta) {
      addDiagnostic(diagnostics, 'LINE_TIMING_OVERLAP', 'warning', {
        lineIndex,
        overlapMs: previousEnd - startMs,
      });
    }
    inspectWords(line, lineIndex, diagnostics);
    previousStart = startMs;
    previousEnd = endMs;
  });
}

function inspectWords(line, lineIndex, diagnostics) {
  let previousStart = null;
  let previousEnd = null;
  const lineStart = Number(line.startMs);
  const lineEnd = lineEndMs(line);

  (line.words || []).forEach((word, wordIndex) => {
    const startMs = Number(word.startMs);
    const durationMs = Number(word.durationMs);
    const endMs = Number(word.endMs);
    const context = { lineIndex, wordIndex };

    if (!Number.isFinite(startMs)) {
      addDiagnostic(diagnostics, 'WORD_TIMING_MISSING', 'warning', context);
      return;
    }
    if (!Number.isFinite(durationMs) || durationMs <= 0 || endMs <= startMs) {
      addDiagnostic(diagnostics, 'WORD_DURATION_INVALID', 'warning', context);
    }
    if (previousStart !== null && startMs < previousStart) {
      addDiagnostic(diagnostics, 'WORD_TIMING_OUT_OF_ORDER', 'warning', context);
    }
    if (previousEnd !== null && startMs < previousEnd) {
      addDiagnostic(diagnostics, 'WORD_TIMING_OVERLAP', 'warning', {
        ...context,
        overlapMs: previousEnd - startMs,
      });
    }
    if (startMs < lineStart || endMs > lineEnd + 50) {
      addDiagnostic(diagnostics, 'WORD_TIMING_OUTSIDE_LINE', 'warning', context);
    }
    previousStart = startMs;
    previousEnd = endMs;
  });
}

function inspectAnnotations(lines, annotations, diagnostics) {
  if (!annotations.length) {
    return;
  }
  const startMs = lines[0]?.startMs;
  const endMs = lines.reduce(
    (maximum, line) => Math.max(maximum, lineEndMs(line)),
    0,
  );
  annotations.forEach((annotation, annotationIndex) => {
    if (
      !Number.isFinite(Number(annotation.startMs))
      || annotation.startMs < startMs
      || annotation.endMs > endMs + 600
    ) {
      addDiagnostic(
        diagnostics,
        'ANNOTATION_TIMING_OUTSIDE_LYRIC',
        'warning',
        { annotationIndex },
      );
    }
  });
}

function hasExplicitLineTiming(line) {
  return line?.timingExplicit !== false && Number.isFinite(Number(line?.startMs));
}

function hasUsableWordTiming(line) {
  return (line.words || []).some((word) => (
    Number.isFinite(Number(word.startMs))
    && Number(word.durationMs) > 0
  ));
}

function hasTranslation(line) {
  return Boolean(line.translation || line.englishTranslation);
}

function hasReading(line) {
  return Boolean(line.reading || line.romanized);
}

function lineEndMs(line) {
  const endMs = Number(line.endMs);
  return Number.isFinite(endMs)
    ? endMs
    : Number(line.startMs) + Math.max(0, Number(line.durationMs) || 0);
}

function addDiagnostic(diagnostics, code, severity, context = {}) {
  diagnostics.push({ code, severity, ...context });
}

function dedupeDiagnostics(diagnostics) {
  const seen = new Set();
  return diagnostics.filter((item) => {
    const key = JSON.stringify(item);
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
}
