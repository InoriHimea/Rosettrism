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
  const annotationLine = inLine ? activeLine : null;
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
              <b>{annotation.symbol}</b>
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
              {index === activeLineIndex && annotationLine?.id === line.id && line.annotations.length > 0 ? (
                <span className="lyric-annotation-row">
                  {activeAnnotations(line, currentMs).map((annotation) => (
                    <span className={`lyric-annotation-chip ${annotation.className}`} key={annotation.id} title={t[annotation.labelKey] || annotation.type}>
                      {annotation.text || t[annotation.labelKey] || annotation.type}
                    </span>
                  ))}
                </span>
              ) : null}
              {index === activeLineIndex && !inLine ? <span className="lyric-gap-dots">•••</span> : null}
              <LineText line={line} currentMs={currentMs} active={index === activeLineIndex && inLine} />
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

function LineText({ line, currentMs, active }) {
  if (!line.words.length) {
    const progress = active ? lyricLineProgress(line, currentMs) : 0;
    return (
      <span className="lyric-line-text lyric-progress-text" style={{ '--lyric-progress': progress }}>
        <span className="lyric-progress-base">{line.text || '· · ·'}</span>
        <span className="lyric-progress-fill" aria-hidden="true">{line.text || '· · ·'}</span>
      </span>
    );
  }
  return (
    <span className="lyric-words">
      {line.words.map((word) => {
        const progress = active ? wordProgress(word, currentMs) : 0;
        return (
          <span className="lyric-word lyric-progress-text" style={{ '--lyric-progress': progress }} key={word.id}>
            <span className="lyric-progress-base">{word.text}</span>
            <span className="lyric-progress-fill" aria-hidden="true">{word.text}</span>
          </span>
        );
      })}
    </span>
  );
}

function activeAnnotations(line, currentMs) {
  const active = line.annotations.filter((annotation) => currentMs >= annotation.startMs - 450 && currentMs <= annotation.endMs + 450);
  return active.length ? active : line.annotations;
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
