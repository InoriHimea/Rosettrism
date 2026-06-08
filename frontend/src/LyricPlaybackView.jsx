import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Pause, Play, RotateCcw } from 'lucide-react';
import {
  findActiveLineIndex,
  formatPlaybackTime,
  resolveLyricGradient,
} from './lyricPlayback.js';

export function LyricPlaybackView({ lyric, settings, t }) {
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentMs, setCurrentMs] = useState(0);
  const [translationMode, setTranslationMode] = useState('off');
  const frameRef = useRef(null);
  const startedAtRef = useRef(0);
  const linesRef = useRef(null);
  const lineRefs = useRef(new Map());
  const durationMs = Math.max(lyric.durationMs || 0, 1000);
  const renderMode = settings.renderMode === 'vertical' ? 'vertical' : 'karaoke';
  const annotationTypes = [...new Map(lyric.annotations.map((annotation) => [annotation.type, annotation])).values()];
  const headingTitle = lyric.displayTitle || lyric.title || t.preview;
  const artistLabel = displayTitleIncludesArtist(headingTitle, lyric.artist)
    ? ''
    : formatArtistLabel(lyric.artist, lyric.artistAlias);
  const metaLines = lyric.lines.filter((line) => line.isMeta);
  const staticMetaLines = lyric.lines.filter((line) => line.isMeta && !hasUsableLineTime(line));
  const bodyLines = lyric.lines.filter((line) => !line.isMeta);
  const introMetaLines = buildIntroMetaLines(metaLines);
  const flowLines = [...introMetaLines, ...bodyLines].sort((a, b) => a.startMs - b.startMs);
  const hasTranslations = bodyLines.some((line) => hasLineTranslation(line));
  const candidateBodyIndex = findActiveLineIndex(bodyLines, currentMs);
  const candidateBodyLine = candidateBodyIndex >= 0 ? bodyLines[candidateBodyIndex] : null;
  const activeBodyIndex = candidateBodyLine && currentMs >= candidateBodyLine.startMs && currentMs < candidateBodyLine.endMs
    ? candidateBodyIndex
    : -1;
  const activeBodyLine = activeBodyIndex >= 0 ? bodyLines[activeBodyIndex] : null;
  const activeMetaIndex = activeBodyLine ? -1 : findActiveTimedLineIndex(introMetaLines, currentMs);
  const activeMetaLine = activeMetaIndex >= 0 ? introMetaLines[activeMetaIndex] : null;
  const activeFlowLine = activeBodyLine || activeMetaLine;
  const activeFlowIndex = activeFlowLine ? flowLines.findIndex((line) => line.id === activeFlowLine.id) : -1;
  const nextBodyIndex = bodyLines.findIndex((line) => line.startMs > currentMs);
  const nextBodyLine = nextBodyIndex >= 0 ? bodyLines[nextBodyIndex] : null;
  const nextFlowIndex = flowLines.findIndex((line) => line.startMs > currentMs);
  const nextFlowLine = nextFlowIndex >= 0 ? flowLines[nextFlowIndex] : null;
  const countdown = lyricCountdown(bodyLines, currentMs);
  const showCountdown = countdown.visible;
  const countdownBodyIndex = countdown.targetLineId
    ? bodyLines.findIndex((line) => line.id === countdown.targetLineId)
    : -1;
  const visibleFlowLine = activeFlowLine || nextFlowLine || flowLines[flowLines.length - 1];
  const visibleBodyLine = activeBodyLine || nextBodyLine || bodyLines[bodyLines.length - 1];
  const currentStripText = activeFlowLine?.text || visibleBodyLine?.text;
  const focusFlowIndex = activeFlowIndex >= 0 ? activeFlowIndex : nextFlowIndex >= 0 ? nextFlowIndex : flowLines.length - 1;
  const initialFlowLine = flowLines[0];
  const scrollTargetId = currentMs <= 0
    ? initialFlowLine?.id
    : (activeFlowLine ? activeFlowLine.id : visibleFlowLine?.id);

  useEffect(() => {
    if (!isPlaying) {
      return undefined;
    }

    startedAtRef.current = performance.now() - currentMs;
    function tick(now) {
      const next = Math.min(now - startedAtRef.current, durationMs);
      setCurrentMs(next);
      if (next >= durationMs) {
        setIsPlaying(false);
        return;
      }
      frameRef.current = requestAnimationFrame(tick);
    }
    frameRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frameRef.current);
  }, [isPlaying, durationMs]);

  useLayoutEffect(() => {
    const container = linesRef.current;
    const node = scrollTargetId ? lineRefs.current.get(scrollTargetId) : null;
    if (!container || !node) {
      return;
    }
    const target = node.offsetTop - container.clientHeight * 0.46 + node.clientHeight / 2;
    container.scrollTop = Math.max(0, target);
  }, [scrollTargetId, renderMode]);

  function seek(nextMs) {
    const next = Math.max(0, Math.min(Number(nextMs), durationMs));
    startedAtRef.current = performance.now() - next;
    setCurrentMs(next);
  }

  function restart() {
    seek(0);
    setIsPlaying(true);
  }

  function cycleTranslationMode() {
    setTranslationMode((mode) => (mode === 'off' ? 'translation' : mode === 'translation' ? 'bilingual' : 'off'));
  }

  function bindLineRef(id) {
    return (node) => {
      if (node) {
        lineRefs.current.set(id, node);
      } else {
        lineRefs.current.delete(id);
      }
    };
  }

  const motionPreset = settings.motionPreset || 'cinematic';
  const lowDistraction = Boolean(settings.lowDistraction);
  const ambientEffects = settings.ambientEffects !== false;

  const style = {
    '--lyric-solid-color': settings.solidColor,
    '--lyric-gradient': resolveLyricGradient(settings.colorPreset),
    '--lyric-stage-background': settings.stageBackgroundColor || '#ffe58b',
  };

  return (
    <section className={`lyric-playback-card lyric-color-${settings.colorMode}`} style={style}>
      <div className="lyric-playback-head">
        <div>
          <span>{t.playback}</span>
          <h4>{headingTitle}</h4>
          <p>{[artistLabel, lyric.source, lyric.inputFormat].filter(Boolean).join(' / ')}</p>
        </div>
        <span className={`lyric-playback-status ${lyric.annotations.length ? 'status-fresh' : ''}`}>
          {lyric.annotations.length ? t.annotationsAvailable : t.annotationsUnavailable}
        </span>
      </div>

      {annotationTypes.length > 0 ? (
        <div className="lyric-annotation-legend" aria-label={t.annotations}>
          {annotationTypes.map((annotation) => (
            <span className={`lyric-annotation-chip ${annotation.className}`} key={annotation.type}>
              <AnnotationGlyph type={annotation.type} />
              <span>{annotationLegendLabel(annotation, t)}</span>
            </span>
          ))}
        </div>
      ) : null}

      <div className={`lyric-stage lyric-stage-qq lyric-stage-${renderMode} lyric-motion-${motionPreset}${lowDistraction ? ' lyric-low-distraction' : ''}${ambientEffects ? ' lyric-ambient-on' : ' lyric-ambient-off'}`} data-testid="karaoke-stage">
        {ambientEffects ? <LyricAmbientEffects /> : null}
        {renderMode === 'karaoke' ? (
          <KaraokeStage
            flowLines={flowLines}
            currentMs={currentMs}
            activeFlowLine={activeFlowLine}
            focusFlowIndex={focusFlowIndex}
            showCountdown={showCountdown}
            countdown={countdown}
            translationMode={translationMode}
            seek={seek}
            bindLineRef={bindLineRef}
            linesRef={linesRef}
            t={t}
          />
        ) : (
          <div className="lyric-lines" ref={linesRef} aria-live="polite">
            {flowLines.map((line, flowIndex) => {
              const isActive = activeFlowLine?.id === line.id;
              const countdownBeforeLine = showCountdown && countdown.targetLineId === line.id;
              return (
                <React.Fragment key={line.id}>
                  {countdownBeforeLine ? (
                    <CountdownRow countdown={countdown} refCallback={bindLineRef(`countdown-${line.id}`)} />
                  ) : null}
                  <button
                    className={lineClassName(line, currentMs, isActive, flowIndex, focusFlowIndex)}
                    type="button"
                    onClick={() => seek(line.startMs)}
                    ref={bindLineRef(line.id)}
                  >
                    <LineText line={line} currentMs={currentMs} active={isActive} translationMode={translationMode} t={t} />
                    {!line.isMeta ? <LineSubtext line={line} translationMode={translationMode} /> : null}
                  </button>
                </React.Fragment>
              );
            })}
          </div>
        )}
        <div className={`lyric-current-strip${countdown.flashing ? ' lyric-dots-flashing' : ''}`} aria-live="polite">
          {currentStripText || '•••'}
        </div>
      </div>

      <div className="lyric-controls">
        <button className="button-primary" type="button" onClick={() => setIsPlaying((playing) => !playing)}>
          {isPlaying ? <Pause size={18} /> : <Play size={18} />}
          {isPlaying ? t.pause : t.play}
        </button>
        <button className="button-secondary" type="button" onClick={restart}>
          <RotateCcw size={18} />
          {t.restart}
        </button>
        <button
          className={`button-secondary lyric-translation-toggle${translationMode !== 'off' ? ' active' : ''}`}
          type="button"
          onClick={cycleTranslationMode}
          disabled={!hasTranslations}
          aria-pressed={translationMode !== 'off'}
        >
          {translationModeLabel(translationMode, t)}
        </button>
        <div className="lyric-time">
          {formatPlaybackTime(currentMs)} / {formatPlaybackTime(durationMs)}
        </div>
        <label className="lyric-seek">
          <span>{t.timeline}</span>
          <input
            type="range"
            min="0"
            max={durationMs}
            value={Math.round(currentMs)}
            onChange={(event) => seek(event.target.value)}
            aria-label={t.timeline}
            aria-valuetext={formatPlaybackTime(currentMs)}
          />
          {lyric.annotations.length > 0 ? (
            <span className="lyric-timeline" aria-hidden="true">
              {lyric.annotations.map((annotation) => (
                <i
                  className={`lyric-timeline-marker ${annotation.className}`}
                  key={annotation.id}
                  style={{ left: `${Math.min(100, Math.max(0, (annotation.startMs / durationMs) * 100))}%` }}
                />
              ))}
            </span>
          ) : null}
        </label>
      </div>
    </section>
  );
}


function LyricAmbientEffects() {
  return (
    <div className="lyric-ambient" aria-hidden="true">
      <span className="lyric-ambient-glow lyric-ambient-glow-a" />
      <span className="lyric-ambient-glow lyric-ambient-glow-b" />
      <span className="lyric-particle lyric-particle-1" />
      <span className="lyric-particle lyric-particle-2" />
      <span className="lyric-particle lyric-particle-3" />
      <span className="lyric-particle lyric-particle-4" />
    </div>
  );
}

function CountdownRow({ countdown, refCallback }) {
  return (
    <div
      className={`lyric-line lyric-line-countdown lyric-line-distance-0${countdown.flashing ? ' lyric-dots-flashing' : ''}${countdown.exiting ? ' lyric-dots-exiting' : ''}`}
      ref={refCallback || null}
    >
      <CountdownDots count={countdown.count} />
    </div>
  );
}

function CountdownDots({ count }) {
  return (
    <span className="lyric-gap-dots" aria-label="•••">
      {[0, 1, 2].map((index) => (
        <span className={index >= count ? 'lyric-gap-dot lyric-gap-dot-hidden' : 'lyric-gap-dot'} key={index}>•</span>
      ))}
    </span>
  );
}

function KaraokeStage({
  flowLines,
  currentMs,
  activeFlowLine,
  focusFlowIndex,
  showCountdown,
  countdown,
  translationMode,
  seek,
  bindLineRef,
  linesRef,
  t,
}) {
  return (
    <div className="lyric-karaoke-lines" ref={linesRef} aria-live="polite">
      {flowLines.map((line, flowIndex) => {
        const isActive = activeFlowLine?.id === line.id;
        const countdownBeforeLine = showCountdown && countdown.targetLineId === line.id;
        const laneClass = flowIndex % 2 === 0 ? 'lyric-karaoke-line-left' : 'lyric-karaoke-line-right';
        return (
          <React.Fragment key={line.id}>
            {countdownBeforeLine ? (
              <CountdownRow countdown={countdown} refCallback={bindLineRef(`countdown-${line.id}`)} />
            ) : null}
            <button
              className={`${lineClassName(line, currentMs, isActive, flowIndex, focusFlowIndex)} lyric-karaoke-line ${laneClass}`}
              type="button"
              onClick={() => seek(line.startMs)}
              ref={bindLineRef(line.id)}
            >
              <LineText line={line} currentMs={currentMs} active={isActive} translationMode={translationMode} t={t} />
              <LineSubtext line={line} translationMode={translationMode} />
            </button>
          </React.Fragment>
        );
      })}
    </div>
  );
}

function LineText({ line, currentMs, active, translationMode, t }) {
  if (translationMode === 'translation' && hasLineTranslation(line)) {
    return <span className="lyric-line-text lyric-translation-primary">{line.translation || line.englishTranslation}</span>;
  }

  const wordAnnotationIds = new Set();
  for (const word of line.words) {
    for (const annotation of word.annotations || []) {
      wordAnnotationIds.add(annotation.id);
    }
  }
  const orphanAnnotations = (line.annotations || []).filter((annotation) => !wordAnnotationIds.has(annotation.id));

  if (line.isMeta) {
    return <span className="lyric-line-text lyric-meta-text">{line.text || '· · ·'}</span>;
  }

  if (!line.words.length) {
    const progress = active ? lyricLineProgress(line, currentMs) : 0;
    const text = line.text || '· · ·';
    const lineAnnotations = line.annotations || [];
    const content = line.ruby?.length ? <RubyText text={text} ruby={line.ruby} /> : text;
    return (
      <span className={`lyric-line-text lyric-progress-text${line.ruby?.length ? ' lyric-ruby-text' : ''}`} style={lyricProgressStyle(progress)}>
        {lineAnnotations.length > 0 ? <AnnotationLayer annotations={lineAnnotations} active={active} t={t} /> : null}
        <span className="lyric-progress-base">{content}</span>
        <span className="lyric-progress-fill" aria-hidden="true">{content}</span>
      </span>
    );
  }

  return (
    <span className="lyric-words">
      {line.words.map((word, wordIndex) => {
        const progress = active ? wordProgress(word, currentMs) : 0;
        const annotations = visibleWordAnnotations(line.words, word, wordIndex);
        const wordState = progress >= 1 ? ' lyric-word-complete' : progress > 0 ? ' lyric-word-current' : '';
        return (
          <span
            className={`lyric-word lyric-progress-text${annotations.length ? ' lyric-word-annotated' : ''}${wordState}`}
            style={lyricProgressStyle(progress, true)}
            key={word.id}
          >
            {annotations.length > 0 ? <AnnotationLayer annotations={annotations} active={active} t={t} /> : null}
            <span className="lyric-progress-base">{word.text}</span>
            <span className="lyric-progress-fill" aria-hidden="true">{word.text}</span>
          </span>
        );
      })}
      {orphanAnnotations.length > 0 ? (
        <span className="lyric-word lyric-word-orphan" aria-hidden="true">
          <AnnotationLayer annotations={orphanAnnotations} active={active} t={t} />
          <span className="lyric-progress-base">&nbsp;</span>
        </span>
      ) : null}
    </span>
  );
}

function RubyText({ text, ruby }) {
  const chars = Array.from(text || '');
  const nodes = [];
  let cursor = 0;
  for (const span of ruby || []) {
    const start = Math.max(0, Math.min(chars.length, Number(span.startChar ?? span.start_char)));
    const end = Math.max(start, Math.min(chars.length, Number(span.endChar ?? span.end_char)));
    if (!Number.isFinite(start) || !Number.isFinite(end) || end <= cursor) {
      continue;
    }
    if (start > cursor) {
      nodes.push(chars.slice(cursor, start).join(''));
    }
    const base = chars.slice(start, end).join('') || span.text || '';
    nodes.push(
      <ruby key={`${start}-${end}-${span.reading}`}>
        <span>{base}</span>
        <rt>{span.reading}</rt>
      </ruby>,
    );
    cursor = end;
  }
  if (cursor < chars.length) {
    nodes.push(chars.slice(cursor).join(''));
  }
  return nodes.length ? nodes : text;
}

function formatArtistLabel(artist, alias) {
  const cleanArtist = String(artist || '').trim();
  const cleanAlias = String(alias || '').trim();
  if (!cleanArtist) {
    return cleanAlias;
  }
  return cleanAlias && cleanAlias !== cleanArtist ? `${cleanArtist}（${cleanAlias}）` : cleanArtist;
}

function displayTitleIncludesArtist(title, artist) {
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

function visibleWordAnnotations(words, word, wordIndex) {
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

function LineSubtext({ line, translationMode }) {
  const translation = line.translation || line.englishTranslation || '';
  const reading = line.reading || line.romanized || '';
  if (translationMode === 'bilingual' && translation) {
    return (
      <>
        <small className="lyric-line-translation">{translation}</small>
        {reading ? <small className="lyric-line-reading">{reading}</small> : null}
      </>
    );
  }
  if (translationMode === 'translation' && reading) {
    return <small className="lyric-line-reading">{reading}</small>;
  }
  return reading ? <small className="lyric-line-reading">{reading}</small> : null;
}

function hasLineTranslation(line) {
  return Boolean(line.translation || line.englishTranslation);
}

function translationModeLabel(mode, t) {
  if (mode === 'translation') {
    return t.lyricTranslationOnly || '译文';
  }
  if (mode === 'bilingual') {
    return t.lyricTranslationBilingual || '双语';
  }
  return t.lyricTranslationOff || '原文';
}

function annotationLegendLabel(annotation, t) {
  const label = t[annotation.labelKey] || annotation.type;
  return `${annotationGlyphText(annotation.type)}=${label}`;
}

function annotationGlyphText(type) {
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

function AnnotationLayer({ annotations, active, t }) {
  const unique = [...new Map(annotations.map((annotation) => [annotation.type, annotation])).values()];
  return (
    <span className="lyric-annotation-layer" aria-hidden="true">
      {unique.map((annotation, index) => {
        const label = t ? t[annotation.labelKey] || annotation.type : annotation.type;
        const style = { '--annotation-index': String(index) };
        return (
          <span
            key={annotation.id}
            className={`lyric-annotation-mark ${annotation.className}`}
            style={style}
            title={annotation.text ? `${label}: ${annotation.text}` : label}
          >
            {active ? <span className="lyric-annotation-text lyric-annotation-label">{label}</span> : null}
            <AnnotationGlyph type={annotation.type} />
          </span>
        );
      })}
    </span>
  );
}

function AnnotationGlyph({ type }) {
  switch (type) {
    case 'stress':
      return <span className="annotation-glyph annotation-glyph-text">·</span>;
    case 'breath':
      return <span className="annotation-glyph annotation-glyph-text">V</span>;
    case 'long_tone':
      return <span className="annotation-glyph annotation-glyph-text">_</span>;
    case 'portamento_up':
      return (
        <svg className="annotation-glyph" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
          <path d="M2 9 L 6 3 L 10 9" fill="none" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case 'portamento_down':
      return (
        <svg className="annotation-glyph" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
          <path d="M2 3 L 6 9 L 10 3" fill="none" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    default:
      return (
        <svg className="annotation-glyph" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
          <circle cx="6" cy="6" r="2" />
        </svg>
      );
  }
}

function lyricProgressStyle(progress, exact = false) {
  const safeProgress = clampProgress(progress);
  const fillEnd = exact || safeProgress === 0 || safeProgress === 1
    ? safeProgress * 100
    : Math.min(100, safeProgress * 100 + 2.4);
  return {
    '--lyric-progress': String(safeProgress),
    '--lyric-fill-end': `${fillEnd}%`,
  };
}

function lyricLineProgress(line, currentMs) {
  const start = line.startMs;
  const end = Math.max(line.endMs || start + 1, start + 1);
  return clampProgress((currentMs - start) / (end - start));
}

function wordProgress(word, currentMs) {
  const start = word.startMs;
  const end = Math.max(word.endMs || start + 1, start + 1);
  return clampProgress((currentMs - start) / (end - start));
}

function clampProgress(value) {
  return Math.max(0, Math.min(1, value));
}

function lineClassName(line, currentMs, active, bodyIndex, focusBodyIndex) {
  const state = active ? 'lyric-line-active' : line.endMs <= currentMs ? 'lyric-line-past' : 'lyric-line-future';
  const distance = focusBodyIndex < 0 ? 4 : Math.min(4, Math.abs(bodyIndex - focusBodyIndex));
  return `lyric-line ${state} lyric-line-distance-${distance}${line?.isMeta ? ' lyric-line-meta' : ''}`;
}

function hasUsableLineTime(line) {
  return Number.isFinite(line.startMs) && line.startMs > 0;
}

function findActiveTimedLineIndex(lines, currentMs) {
  return lines.findIndex((line) => currentMs >= line.startMs && currentMs < line.endMs);
}

function buildIntroMetaLines(lines) {
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
    const index = unique.length;
    unique.push({
      ...line,
      id: `intro-${line.id}`,
      startMs: index * 2000,
      durationMs: 2000,
      endMs: (index + 1) * 2000,
      isMeta: true,
      words: [],
      annotations: [],
    });
  }
  return unique;
}

function firstMetaTitleText(lines) {
  return String(lines.find((line) => String(line.id || '').startsWith('meta-title-'))?.text || '').trim();
}

function normalizeMetaText(text) {
  return String(text || '')
    .replace(/（[^）]*）|\([^)]*\)/g, '')
    .replace(/([㐀-鿿])([A-Za-z][\w .'-]*)$/u, '$1')
    .replace(/[\s—–－-]+/g, '')
    .trim()
    .toLowerCase();
}

function looksLikeTitleDuplicate(text, titleText) {
  const normalized = normalizeMetaText(text);
  const title = normalizeMetaText(titleText);
  return Boolean(title && (normalized === title || title.includes(normalized) || normalized.includes(title)));
}

function lyricCountdown(lines, currentMs) {
  const exitingLine = lines.find((line, index) => countdownEligible(lines, index) && currentMs >= line.startMs && currentMs < line.startMs + 420);
  if (exitingLine) {
    return {
      count: 1,
      flashing: false,
      exiting: true,
      remainingMs: Math.max(0, exitingLine.startMs + 420 - currentMs),
      targetLineId: exitingLine.id,
      visible: true,
    };
  }

  const nextIndex = lines.findIndex((line) => line.startMs > currentMs);
  const nextLine = nextIndex >= 0 ? lines[nextIndex] : null;
  if (!nextLine) {
    return { count: 0, flashing: false, exiting: false, remainingMs: 0, targetLineId: null, visible: false };
  }

  const remainingMs = nextLine.startMs - currentMs;
  if (!countdownEligible(lines, nextIndex)) {
    return { count: 0, flashing: false, exiting: false, remainingMs, targetLineId: nextLine.id, visible: false };
  }
  if (remainingMs <= 1200) {
    return { count: 1, flashing: false, exiting: false, remainingMs, targetLineId: nextLine.id, visible: true };
  }
  if (remainingMs <= 2200) {
    return { count: 2, flashing: false, exiting: false, remainingMs, targetLineId: nextLine.id, visible: true };
  }
  return { count: 3, flashing: remainingMs > 3200, exiting: false, remainingMs, targetLineId: nextLine.id, visible: true };
}

function countdownEligible(lines, index) {
  if (index < 0 || !lines[index]) {
    return false;
  }
  if (index === 0) {
    return lines[index].startMs >= 3200;
  }
  const previous = lines[index - 1];
  return lines[index].startMs - previous.startMs >= 9000;
}
