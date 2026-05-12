import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Pause, Play, RotateCcw } from 'lucide-react';
import {
  findActiveLineIndex,
  findVisibleLineIndex,
  formatPlaybackTime,
  resolveLyricGradient,
} from './lyricPlayback.js';

export function LyricPlaybackView({ lyric, settings, t }) {
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentMs, setCurrentMs] = useState(0);
  const frameRef = useRef(null);
  const startedAtRef = useRef(0);
  const linesRef = useRef(null);
  const lineRefs = useRef(new Map());
  const reducedMotion = useMemo(() => window.matchMedia?.('(prefers-reduced-motion: reduce)').matches || false, []);
  const durationMs = Math.max(lyric.durationMs || 0, 1000);
  const activeLineIndex = findActiveLineIndex(lyric.lines, currentMs);
  const visibleLineIndex = findVisibleLineIndex(lyric.lines, currentMs);
  const activeLine = lyric.lines[activeLineIndex];
  const visibleLine = lyric.lines[visibleLineIndex];
  const inLine = activeLine && currentMs >= activeLine.startMs && currentMs < activeLine.endMs;
  const annotationTypes = [...new Map(lyric.annotations.map((annotation) => [annotation.type, annotation])).values()];
  const lineProgress = inLine ? lyricLineProgress(activeLine, currentMs) : 0;

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
    const node = visibleLine ? lineRefs.current.get(visibleLine.id) : null;
    if (!container || !node) {
      return;
    }
    const target = node.offsetTop - container.clientHeight / 3 + node.clientHeight * (0.35 + lineProgress * 0.3);
    container.scrollTo({
      top: Math.max(0, target),
      behavior: reducedMotion || isPlaying ? 'auto' : 'smooth',
    });
  }, [visibleLine?.id, lineProgress, isPlaying, reducedMotion]);

  function seek(nextMs) {
    setCurrentMs(Math.max(0, Math.min(Number(nextMs), durationMs)));
  }

  function restart() {
    seek(0);
    setIsPlaying(true);
  }

  const style = {
    '--lyric-solid-color': settings.solidColor,
    '--lyric-gradient': resolveLyricGradient(settings.colorPreset),
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

      <div className="lyric-current-strip" aria-live="polite">
        {inLine ? activeLine?.text : '•••'}
      </div>

      <div className="lyric-stage lyric-stage-qq">
        <div className="lyric-lines" ref={linesRef}>
          {lyric.lines.map((line, index) => (
            <button
              className={lineClassName(index, activeLineIndex)}
              key={line.id}
              type="button"
              onClick={() => seek(line.startMs)}
              ref={(node) => {
                if (node) {
                  lineRefs.current.set(line.id, node);
                } else {
                  lineRefs.current.delete(line.id);
                }
              }}
            >
              {index === activeLineIndex && !inLine ? <span className="lyric-gap-dots">•••</span> : null}
              <LineText line={line} currentMs={currentMs} active={index === activeLineIndex && inLine} t={t} />
              {line.reading || line.romanized ? <small>{line.reading || line.romanized}</small> : null}
            </button>
          ))}
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

function LineText({ line, currentMs, active, t }) {
  // Build a lookup: annotations that couldn't attach to a specific word
  const wordAnnotationIds = new Set();
  for (const word of line.words) {
    for (const annotation of word.annotations || []) {
      wordAnnotationIds.add(annotation.id);
    }
  }
  const orphanAnnotations = (line.annotations || []).filter((annotation) => !wordAnnotationIds.has(annotation.id));

  if (!line.words.length) {
    const progress = active ? lyricLineProgress(line, currentMs) : 0;
    const text = line.text || '· · ·';
    // With no word timing, overlay any line-level annotations at the text start
    const lineAnnotations = line.annotations || [];
    return (
      <span className="lyric-line-text lyric-progress-text" style={{ '--lyric-progress': progress }}>
        {lineAnnotations.length > 0 ? (
          <AnnotationLayer annotations={lineAnnotations} t={t} />
        ) : null}
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
        return (
          <span
            className={`lyric-word lyric-progress-text${annotations.length ? ' lyric-word-annotated' : ''}`}
            style={{ '--lyric-progress': progress }}
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

function AnnotationLayer({ annotations, t }) {
  // De-duplicate by type so we don't stack identical marks on a single character
  const unique = [...new Map(annotations.map((annotation) => [annotation.type, annotation])).values()];
  return (
    <span className="lyric-annotation-layer" aria-hidden="true">
      {unique.map((annotation) => (
        <span
          key={annotation.id}
          className={`lyric-annotation-mark ${annotation.className}`}
          title={t ? t[annotation.labelKey] || annotation.type : annotation.type}
        >
          <AnnotationGlyph type={annotation.type} />
        </span>
      ))}
    </span>
  );
}

function AnnotationGlyph({ type }) {
  // SVG glyphs mirroring QQ Music's small inline marks (重音/换气/长音/上滑/下滑)
  switch (type) {
    case 'stress':
      // Small filled dot with a subtle halo — "accent mark"
      return (
        <svg className="annotation-glyph" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
          <circle cx="6" cy="6" r="3" />
        </svg>
      );
    case 'breath':
      // Comma-like breath mark (QQ uses a small apostrophe / check shape)
      return (
        <svg className="annotation-glyph" viewBox="0 0 12 12" aria-hidden="true" focusable="false">
          <path d="M7 2 C 7 2, 4 3.2, 4 5.4 C 4 6.8, 5.2 7.6, 6.2 7.6 L 5.2 10.6" fill="none" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case 'long_tone':
      // Long horizontal bar
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

function lineClassName(index, activeLineIndex) {
  const state = index === activeLineIndex ? 'lyric-line-active' : index < activeLineIndex ? 'lyric-line-past' : 'lyric-line-future';
  return `lyric-line ${state}`;
}
