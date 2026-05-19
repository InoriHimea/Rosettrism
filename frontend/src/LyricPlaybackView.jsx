import React, { useEffect, useMemo, useRef, useState } from 'react';
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
  const reducedMotion = useMemo(() => window.matchMedia?.('(prefers-reduced-motion: reduce)').matches || false, []);
  const durationMs = Math.max(lyric.durationMs || 0, 1000);
  const renderMode = settings.renderMode === 'karaoke' ? 'karaoke' : 'vertical';
  const activeLineIndex = findActiveLineIndex(lyric.lines, currentMs);
  const activeLine = lyric.lines[activeLineIndex];
  const inLine = Boolean(activeLine && currentMs >= activeLine.startMs && currentMs < activeLine.endMs);
  const annotationTypes = [...new Map(lyric.annotations.map((annotation) => [annotation.type, annotation])).values()];
  const metaLines = lyric.lines.filter((line) => line.isMeta);
  const bodyLines = lyric.lines.filter((line) => !line.isMeta);
  const hasTranslations = bodyLines.some((line) => hasLineTranslation(line));
  const activeBodyIndex = inLine && !activeLine?.isMeta ? bodyLines.findIndex((line) => line.id === activeLine.id) : -1;
  const activeBodyLine = activeBodyIndex >= 0 ? bodyLines[activeBodyIndex] : null;
  const nextBodyIndex = bodyLines.findIndex((line) => line.startMs > currentMs);
  const nextBodyLine = nextBodyIndex >= 0 ? bodyLines[nextBodyIndex] : null;
  const countdown = lyricCountdown(bodyLines, currentMs);
  const showCountdown = shouldShowCountdown(bodyLines, currentMs, activeBodyLine, nextBodyIndex);
  const visibleBodyLine = activeBodyLine || nextBodyLine || bodyLines[bodyLines.length - 1];
  const currentStripText = activeBodyLine?.text || (showCountdown ? countdown.text : visibleBodyLine?.text);
  const focusBodyIndex = activeBodyIndex >= 0 ? activeBodyIndex : nextBodyIndex >= 0 ? nextBodyIndex : bodyLines.length - 1;
  const scrollTargetId = renderMode === 'vertical'
    ? (showCountdown && nextBodyLine ? `countdown-${nextBodyLine.id}` : visibleBodyLine?.id)
    : null;

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

  useEffect(() => {
    const container = linesRef.current;
    const node = scrollTargetId ? lineRefs.current.get(scrollTargetId) : null;
    if (!container || !node) {
      return;
    }
    const target = node.offsetTop - container.clientHeight * 0.46 + node.clientHeight / 2;
    container.scrollTo({
      top: Math.max(0, target),
      behavior: reducedMotion ? 'auto' : 'smooth',
    });
  }, [scrollTargetId, reducedMotion]);

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
          <h4>{lyric.title || t.preview}</h4>
          <p>{[lyric.artist, lyric.source].filter(Boolean).join(' / ')}</p>
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
              {t[annotation.labelKey] || annotation.type}
            </span>
          ))}
        </div>
      ) : null}

      <div className={`lyric-stage lyric-stage-qq lyric-stage-${renderMode}`}>
        {renderMode === 'karaoke' ? (
          <KaraokeStage
            lyric={lyric}
            metaLines={metaLines}
            bodyLines={bodyLines}
            currentMs={currentMs}
            activeBodyLine={activeBodyLine}
            activeBodyIndex={activeBodyIndex}
            nextBodyLine={nextBodyLine}
            nextBodyIndex={nextBodyIndex}
            focusBodyIndex={focusBodyIndex}
            showCountdown={showCountdown}
            countdown={countdown}
            translationMode={translationMode}
            seek={seek}
            t={t}
          />
        ) : (
          <div className="lyric-lines" ref={linesRef} aria-live="polite">
            <StageMeta metaLines={metaLines} refCallback={bindLineRef('stage-meta')} />
            {bodyLines.map((line, bodyIndex) => {
              const isActive = activeBodyLine?.id === line.id;
              const countdownBeforeLine = showCountdown && nextBodyLine?.id === line.id;
              return (
                <React.Fragment key={line.id}>
                  {countdownBeforeLine ? (
                    <CountdownRow countdown={countdown} refCallback={bindLineRef(`countdown-${line.id}`)} />
                  ) : null}
                  <button
                    className={lineClassName(line, currentMs, isActive, bodyIndex, focusBodyIndex)}
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

function StageMeta({ metaLines, refCallback }) {
  if (!metaLines.length) {
    return null;
  }

  return (
    <div className="lyric-stage-meta" ref={refCallback}>
      <div className="lyric-stage-credit-list">
        {metaLines.map((line) => (
          <span key={line.id}>{line.text}</span>
        ))}
      </div>
    </div>
  );
}

function CountdownRow({ countdown, refCallback }) {
  return (
    <div
      className={`lyric-line lyric-line-countdown lyric-line-distance-0${countdown.flashing ? ' lyric-dots-flashing' : ''}`}
      ref={refCallback || null}
    >
      <span className="lyric-gap-dots">{countdown.text || '•••'}</span>
    </div>
  );
}

function KaraokeStage({
  lyric,
  metaLines,
  bodyLines,
  currentMs,
  activeBodyLine,
  activeBodyIndex,
  nextBodyLine,
  nextBodyIndex,
  focusBodyIndex,
  showCountdown,
  countdown,
  translationMode,
  seek,
  t,
}) {
  const metaSlot = karaokeMetaSlot(metaLines, showCountdown, countdown, nextBodyIndex, currentMs);
  const focusIndex = activeBodyIndex >= 0 ? activeBodyIndex : nextBodyIndex >= 0 ? nextBodyIndex : bodyLines.length - 1;
  const nextIndex = focusIndex + 1;
  const leftIndex = showCountdown && nextBodyIndex >= 0
    ? nextBodyIndex
    : focusIndex % 2 === 0
      ? focusIndex
      : nextIndex;
  const rightIndex = showCountdown && nextBodyIndex >= 0
    ? nextBodyIndex + 1
    : focusIndex % 2 === 0
      ? nextIndex
      : focusIndex;
  const leftLine = bodyLines[leftIndex] || null;
  const rightLine = bodyLines[rightIndex] || null;
  const leftSlot = leftLine ? { type: 'line', line: leftLine, bodyIndex: leftIndex, active: activeBodyLine?.id === leftLine.id } : null;
  const rightSlot = rightLine ? { type: 'line', line: rightLine, bodyIndex: rightIndex, active: activeBodyLine?.id === rightLine.id } : null;

  return (
    <div className="lyric-karaoke-lines lyric-karaoke-three-line" aria-live="polite">
      <div className="lyric-karaoke-meta-line">
        {metaSlot.type === 'countdown' ? (
          <span className={`lyric-gap-dots${countdown.flashing ? ' lyric-dots-flashing' : ''}`}>{countdown.text || '•••'}</span>
        ) : (
          <span>{metaSlot.text}</span>
        )}
      </div>
      <div className="lyric-karaoke-lane lyric-karaoke-lane-left lyric-karaoke-lane-current">
        <KaraokeSlot
          slot={leftSlot}
          currentMs={currentMs}
          focusBodyIndex={focusBodyIndex}
          translationMode={translationMode}
          seek={seek}
          t={t}
        />
      </div>
      <div className="lyric-karaoke-lane lyric-karaoke-lane-right lyric-karaoke-lane-next">
        <KaraokeSlot
          slot={rightSlot}
          currentMs={currentMs}
          focusBodyIndex={focusBodyIndex}
          translationMode={translationMode}
          seek={seek}
          t={t}
        />
      </div>
    </div>
  );
}

function karaokeMetaSlot(metaLines, showCountdown, countdown, nextBodyIndex, currentMs) {
  if (showCountdown) {
    return { type: 'countdown', countdown };
  }
  if (!metaLines.length) {
    return { type: 'text', text: '•••' };
  }
  const activeMeta = metaLines.find((line) => currentMs >= line.startMs && currentMs < line.endMs);
  if (activeMeta) {
    return { type: 'text', text: activeMeta.text };
  }
  if (nextBodyIndex <= 0) {
    return { type: 'text', text: metaLines[0].text };
  }
  return { type: 'text', text: metaLines.slice(1).map((line) => line.text).join(' / ') || metaLines[0].text };
}

function KaraokeSlot({ slot, currentMs, focusBodyIndex, translationMode, seek, t }) {
  if (!slot) {
    return <div className="lyric-karaoke-empty">•••</div>;
  }

  if (slot.type === 'countdown') {
    return <CountdownRow countdown={slot.countdown} />;
  }

  const { line, bodyIndex, active } = slot;
  return (
    <button
      className={`${lineClassName(line, currentMs, active, bodyIndex, focusBodyIndex)} lyric-karaoke-line`}
      type="button"
      onClick={() => seek(line.startMs)}
    >
      <LineText line={line} currentMs={currentMs} active={active} translationMode={translationMode} t={t} />
      <LineSubtext line={line} translationMode={translationMode} />
    </button>
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
    return (
      <span className="lyric-line-text lyric-progress-text" style={lyricProgressStyle(progress)}>
        {lineAnnotations.length > 0 ? <AnnotationLayer annotations={lineAnnotations} t={t} /> : null}
        <span className="lyric-progress-base">{text}</span>
        <span className="lyric-progress-fill" aria-hidden="true">{text}</span>
      </span>
    );
  }

  return (
    <span className="lyric-words">
      {line.words.map((word) => {
        const progress = active ? wordProgress(word, currentMs) : 0;
        const annotations = word.annotations || [];
        const wordState = progress >= 1 ? ' lyric-word-complete' : progress > 0 ? ' lyric-word-current' : '';
        return (
          <span
            className={`lyric-word lyric-progress-text${annotations.length ? ' lyric-word-annotated' : ''}${wordState}`}
            style={lyricProgressStyle(progress)}
            key={word.id}
          >
            {annotations.length > 0 ? <AnnotationLayer annotations={annotations} t={t} /> : null}
            <span className="lyric-progress-base">{word.text}</span>
            <span className="lyric-progress-fill" aria-hidden="true">{word.text}</span>
          </span>
        );
      })}
      {orphanAnnotations.length > 0 ? (
        <span className="lyric-word lyric-word-orphan" aria-hidden="true">
          <AnnotationLayer annotations={orphanAnnotations} t={t} />
          <span className="lyric-progress-base">&nbsp;</span>
        </span>
      ) : null}
    </span>
  );
}

function LineSubtext({ line, translationMode }) {
  const translation = line.translation || line.englishTranslation || '';
  const reading = line.reading || line.romanized || '';
  if (translationMode === 'bilingual' && translation) {
    return <small className="lyric-line-translation">{translation}</small>;
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

function AnnotationLayer({ annotations, t }) {
  const unique = [...new Map(annotations.map((annotation) => [annotation.type, annotation])).values()];
  return (
    <span className="lyric-annotation-layer" aria-hidden="true">
      {unique.map((annotation) => {
        const label = t ? t[annotation.labelKey] || annotation.type : annotation.type;
        return (
          <span
            key={annotation.id}
            className={`lyric-annotation-mark ${annotation.className}`}
            title={annotation.text ? `${label}: ${annotation.text}` : label}
          >
            <AnnotationGlyph type={annotation.type} />
            <span className="lyric-annotation-text">{annotation.text || label}</span>
          </span>
        );
      })}
    </span>
  );
}

function AnnotationGlyph({ type }) {
  switch (type) {
    case 'stress':
      return (
        <svg className="annotation-glyph" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
          <circle cx="6" cy="6" r="3" />
        </svg>
      );
    case 'breath':
      return (
        <svg className="annotation-glyph" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
          <path d="M7 2 C 7 2, 4 3.2, 4 5.4 C 4 6.8, 5.2 7.6, 6.2 7.6 L 5.2 10.6" fill="none" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case 'long_tone':
      return (
        <svg className="annotation-glyph" viewBox="0 0 14 12" aria-hidden="true" focusable="false">
          <rect x="1" y="5" width="12" height="2" rx="1" />
        </svg>
      );
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

function lyricProgressStyle(progress) {
  const safeProgress = clampProgress(progress);
  const fillEnd = safeProgress > 0 && safeProgress < 1
    ? Math.min(100, safeProgress * 100 + 2.4)
    : safeProgress * 100;
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

function shouldShowCountdown(lines, currentMs, activeLine, nextIndex) {
  if (activeLine || nextIndex < 0) {
    return false;
  }
  if (nextIndex === 0) {
    return true;
  }
  const previousLine = lines[nextIndex - 1];
  const nextLine = lines[nextIndex];
  return Boolean(previousLine && nextLine && nextLine.startMs - previousLine.endMs >= 9000);
}

function lyricCountdown(lines, currentMs) {
  const nextLine = lines.find((line) => line.startMs > currentMs);
  if (!nextLine) {
    return { text: '•••', flashing: false, remainingMs: 0 };
  }
  const remainingMs = nextLine.startMs - currentMs;
  if (remainingMs <= 1000) {
    return { text: '•', flashing: false, remainingMs };
  }
  if (remainingMs <= 2000) {
    return { text: '••', flashing: false, remainingMs };
  }
  return { text: '•••', flashing: remainingMs > 3000, remainingMs };
}
