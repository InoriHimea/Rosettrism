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
  const frameRef = useRef(null);
  const startedAtRef = useRef(0);
  const linesRef = useRef(null);
  const lineRefs = useRef(new Map());
  const reducedMotion = useMemo(() => window.matchMedia?.('(prefers-reduced-motion: reduce)').matches || false, []);
  const durationMs = Math.max(lyric.durationMs || 0, 1000);
  const activeLineIndex = findActiveLineIndex(lyric.lines, currentMs);
  const activeLine = lyric.lines[activeLineIndex];
  const annotationTypes = [...new Map(lyric.annotations.map((annotation) => [annotation.type, annotation])).values()];

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
  }, [isPlaying, currentMs, durationMs]);

  useEffect(() => {
    const container = linesRef.current;
    const node = activeLine ? lineRefs.current.get(activeLine.id) : null;
    if (!container || !node) {
      return;
    }
    const target = node.offsetTop - container.clientHeight / 3 + node.clientHeight / 2;
    container.scrollTo({
      top: Math.max(0, target),
      behavior: reducedMotion ? 'auto' : 'smooth',
    });
  }, [activeLine?.id, reducedMotion]);

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
        {activeLine?.text || t.preview}
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
              {index === activeLineIndex && line.annotations.length > 0 ? (
                <span className="lyric-annotation-row">
                  {line.annotations.map((annotation) => (
                    <span className={`lyric-annotation-chip ${annotation.className}`} key={annotation.id} title={t[annotation.labelKey] || annotation.type}>
                      {annotation.text || t[annotation.labelKey] || annotation.type}
                    </span>
                  ))}
                </span>
              ) : null}
              <LineText line={line} currentMs={currentMs} active={index === activeLineIndex} />
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
    const sung = active && currentMs > line.startMs;
    return <span className={`lyric-line-text ${sung ? 'lyric-word-sung' : ''}`}>{line.text || '· · ·'}</span>;
  }
  return (
    <span className="lyric-words">
      {line.words.map((word) => {
        const sung = active && currentMs >= word.startMs;
        const singing = active && currentMs >= word.startMs && currentMs < Math.max(word.endMs, word.startMs + 1);
        return (
          <span className={`lyric-word ${sung ? 'lyric-word-sung' : ''} ${singing ? 'lyric-word-active' : ''}`} key={word.id}>
            {word.text}
          </span>
        );
      })}
    </span>
  );
}

function lineClassName(index, activeLineIndex) {
  const state = index === activeLineIndex ? 'lyric-line-active' : index < activeLineIndex ? 'lyric-line-past' : 'lyric-line-future';
  return `lyric-line ${state}`;
}
