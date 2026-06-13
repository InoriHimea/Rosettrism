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
  const bodyLines = lyric.lines.filter((line) => !line.isMeta);
  const firstBodyStartMs = bodyLines[0]?.startMs || 0;
  const introMetaLines = buildIntroMetaLines(metaLines, firstBodyStartMs);
  const introMetaEndMs = introMetaLines.length ? Math.max(...introMetaLines.map((line) => line.endMs || 0)) : 0;
  const flowLines = [...introMetaLines, ...bodyLines];
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
  const countdown = lyricCountdown(bodyLines, currentMs, { introMetaEndMs });
  const showCountdown = countdown.visible;
  const visibleFlowLine = activeFlowLine || nextFlowLine || flowLines[flowLines.length - 1];
  const visibleBodyLine = activeBodyLine || nextBodyLine || bodyLines[bodyLines.length - 1];
  const currentStripText = showCountdown ? '•••' : activeFlowLine?.text || visibleBodyLine?.text;
  const focusFlowIndex = activeFlowIndex >= 0 ? activeFlowIndex : nextFlowIndex >= 0 ? nextFlowIndex : flowLines.length - 1;
  const focusBodyIndex = activeBodyIndex >= 0 ? activeBodyIndex : nextBodyIndex >= 0 ? nextBodyIndex : bodyLines.length - 1;
  const initialFlowLine = flowLines[0];
  const countdownTargetId = showCountdown && countdown.targetLineId ? `countdown-${countdown.targetLineId}` : null;
  const scrollTargetId = currentMs <= 0
    ? initialFlowLine?.id
    : (countdownTargetId || (activeFlowLine ? activeFlowLine.id : visibleFlowLine?.id));

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
    if (renderMode === 'karaoke') {
      if (container) {
        container.scrollTop = 0;
      }
      return;
    }
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
            bodyLines={bodyLines}
            headingTitle={headingTitle}
            currentMs={currentMs}
            activeBodyLine={activeBodyLine}
            activeMetaLine={activeMetaLine}
            introMetaLines={introMetaLines}
            focusBodyIndex={focusBodyIndex}
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

function CountdownRow({ countdown, refCallback, laneClass = '' }) {
  return (
    <div
      className={`lyric-line lyric-line-countdown lyric-line-distance-0 lyric-line-countdown-${countdown.kind || 'gap'}${countdown.flashing ? ' lyric-dots-flashing' : ''}${countdown.exiting ? ' lyric-dots-exiting' : ''}${laneClass ? ` ${laneClass}` : ''}`}
      ref={refCallback || null}
    >
      <CountdownDots count={countdown.count} exiting={countdown.exiting} />
    </div>
  );
}

function CountdownDots({ count, exiting = false }) {
  return (
    <span className="lyric-gap-dots" aria-label="•••">
      {[0, 1, 2].map((index) => (
        <span className={countdownDotClass(index, count, exiting)} key={index} />
      ))}
    </span>
  );
}

function countdownDotClass(index, count, exiting) {
  const activeCount = Math.max(0, Math.min(3, count));
  if (exiting) {
    return index === 0 ? 'lyric-gap-dot lyric-gap-dot-popping' : 'lyric-gap-dot lyric-gap-dot-gone';
  }
  if (index < activeCount) {
    return 'lyric-gap-dot lyric-gap-dot-active';
  }
  if (index === activeCount && activeCount < 3) {
    return 'lyric-gap-dot lyric-gap-dot-popping';
  }
  return 'lyric-gap-dot lyric-gap-dot-gone';
}

function KaraokeStage({
  bodyLines,
  headingTitle,
  currentMs,
  activeBodyLine,
  activeMetaLine,
  introMetaLines,
  focusBodyIndex,
  showCountdown,
  countdown,
  translationMode,
  seek,
  bindLineRef,
  linesRef,
  t,
}) {
  const laneItems = activeMetaLine ? [] : karaokeLaneItems(bodyLines, activeBodyLine, focusBodyIndex, showCountdown, countdown);
  const placeholderLanes = karaokePlaceholderLanes(laneItems);
  return (
    <div className="lyric-karaoke-lines lyric-karaoke-dual-lines" ref={linesRef} aria-live="polite">
      <KaraokeMetaPanel
        lines={introMetaLines}
        currentMs={currentMs}
        headingTitle={headingTitle}
      />
      {laneItems.map((item) => {
        if (item.kind === 'countdown') {
          const { targetLine, bodyIndex } = item;
          const isActive = activeBodyLine?.id === targetLine?.id;
          return (
            <div className={`lyric-karaoke-stack ${item.laneClass} ${item.lanePositionClass}`} key={item.key}>
              <CountdownRow
                countdown={countdown}
                laneClass={`lyric-karaoke-line lyric-karaoke-countdown-above ${item.laneClass} ${item.lanePositionClass}`}
                refCallback={bindLineRef(item.key)}
              />
              {targetLine ? (
                <button
                  className={`${lineClassName(targetLine, currentMs, isActive, bodyIndex, focusBodyIndex)} lyric-karaoke-line lyric-karaoke-countdown-target ${item.laneClass}`}
                  type="button"
                  onClick={() => seek(targetLine.startMs)}
                  ref={bindLineRef(targetLine.id)}
                >
                  <LineText line={targetLine} currentMs={currentMs} active={isActive} translationMode={translationMode} t={t} />
                  <LineSubtext line={targetLine} translationMode={translationMode} />
                </button>
              ) : null}
            </div>
          );
        }
        const { line, bodyIndex } = item;
        const isActive = activeBodyLine?.id === line.id;
        return (
          <button
            className={`${lineClassName(line, currentMs, isActive, bodyIndex, focusBodyIndex)} lyric-karaoke-line ${item.laneClass} ${item.lanePositionClass}`}
            type="button"
            onClick={() => seek(line.startMs)}
            ref={bindLineRef(line.id)}
            key={line.id}
          >
            <LineText line={line} currentMs={currentMs} active={isActive} translationMode={translationMode} t={t} />
            <LineSubtext line={line} translationMode={translationMode} />
          </button>
        );
      })}
      {placeholderLanes.map((laneClass) => (
        <span className={`lyric-karaoke-placeholder ${laneClass}`} aria-hidden="true" key={laneClass} />
      ))}
    </div>
  );
}

function KaraokeMetaPanel({ lines, currentMs, headingTitle }) {
  const titleText = String(headingTitle || '').trim();
  if (!titleText && !lines.length) {
    return null;
  }
  const normalizedTitle = normalizeMetaText(titleText);
  const detailLines = lines.filter((line) => {
    const text = String(line.text || '').trim();
    return text && normalizeMetaText(text) !== normalizedTitle && !looksLikeTitleDuplicate(text, titleText);
  });
  const metaCount = Math.max(1, detailLines.length + 1);
  const detailIndex = detailLines.length ? Math.floor(Math.max(0, currentMs) / 2400) % detailLines.length : -1;
  const activeDetailLine = detailIndex >= 0 ? detailLines[detailIndex] : null;
  return (
    <div className="lyric-karaoke-meta-panel" aria-live="polite">
      <span className="lyric-karaoke-meta-row lyric-karaoke-meta-title-row">
        <span className="lyric-karaoke-meta-index">1/{metaCount}</span>
        <span className="lyric-karaoke-meta-title">{titleText}</span>
      </span>
      {activeDetailLine ? (
        <span className="lyric-karaoke-meta-detail" key={activeDetailLine.id}>
          <span className="lyric-karaoke-meta-index">{detailIndex + 2}/{metaCount}</span>
          <span className="lyric-karaoke-meta-line">{activeDetailLine.text}</span>
        </span>
      ) : null}
    </div>
  );
}

function karaokeLaneItems(bodyLines, activeBodyLine, focusBodyIndex, showCountdown, countdown) {
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

function karaokeLineLaneItem(line, bodyIndex) {
  const topLane = bodyIndex % 2 === 0;
  return {
    kind: 'line',
    line,
    bodyIndex,
    laneClass: topLane ? 'lyric-karaoke-line-left' : 'lyric-karaoke-line-right',
    lanePositionClass: topLane ? 'lyric-karaoke-lane-top' : 'lyric-karaoke-lane-bottom',
  };
}

function karaokePlaceholderLanes(items) {
  const occupied = new Set(items.map((item) => item.lanePositionClass).filter(Boolean));
  return ['lyric-karaoke-lane-top', 'lyric-karaoke-lane-bottom'].filter((laneClass) => !occupied.has(laneClass));
}

function laneSortIndex(lanePositionClass) {
  return lanePositionClass === 'lyric-karaoke-lane-top' ? 0 : 1;
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
        const markerClass = shouldReserveBreathGap(line.words, wordIndex, annotations)
          ? ' lyric-word-has-breath'
          : '';
        return (
          <span
            className={`lyric-word lyric-progress-text${annotations.length ? ' lyric-word-annotated' : ''}${markerClass}${wordState}`}
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

function shouldReserveBreathGap(words, wordIndex, annotations) {
  const hasLeadingBreath = annotations.some((annotation) => annotation.type === 'breath' && annotationAnchorPercent(annotation) <= 12);
  if (!hasLeadingBreath) {
    return false;
  }
  return words.slice(0, wordIndex).some((word) => /\S/.test(String(word.text || '')));
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
  const unique = uniqueAnnotations(annotations);
  const labelState = annotationLabelState(unique);
  return (
    <span className="lyric-annotation-layer" aria-hidden="true">
      {unique.map((annotation, index) => {
        const label = t ? t[annotation.labelKey] || annotation.type : annotation.type;
        const style = {
          '--annotation-index': String(index),
          '--annotation-label-row': String(labelState.rows.get(annotation.id) ?? index),
          '--annotation-x': `${annotationAnchorPercent(annotation)}%`,
        };
        const showLabel = active && labelState.ids.has(annotation.id);
        return (
          <span
            key={annotation.id}
            className={`lyric-annotation-mark ${annotation.className}`}
            style={style}
            title={annotation.text ? `${label}: ${annotation.text}` : label}
          >
            {showLabel ? <span className="lyric-annotation-text lyric-annotation-label">{label}</span> : null}
            <AnnotationGlyph type={annotation.type} />
          </span>
        );
      })}
    </span>
  );
}

function uniqueAnnotations(annotations) {
  const unique = new Map();
  for (const annotation of annotations) {
    const key = annotation.id || `${annotation.type}:${annotation.startMs}:${annotation.text}:${annotation.anchorKey || ''}`;
    if (!unique.has(key)) {
      unique.set(key, annotation);
    }
  }
  return [...unique.values()];
}

function annotationLabelState(annotations) {
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

function annotationAnchorKey(annotation) {
  return annotation.anchorKey || `${Math.round(annotationAnchorPercent(annotation) / 6)}`;
}

function annotationAnchorPercent(annotation) {
  const value = Number(annotation.anchorPercent);
  return Number.isFinite(value) ? Math.max(0, Math.min(100, value)) : 50;
}

function annotationLabelPriority(type) {
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

function findActiveTimedLineIndex(lines, currentMs) {
  return lines.findIndex((line) => currentMs >= line.startMs && currentMs < line.endMs);
}

function buildIntroMetaLines(lines, firstBodyStartMs = 0) {
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

function introMetaDurationMs(count, firstBodyStartMs) {
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

function lyricCountdown(lines, currentMs, { introMetaEndMs = 0 } = {}) {
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

function countdownSeparatorKind(lines, index, introMetaEndMs = 0) {
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
  const previous = lines[index - 1];
  const previousEndMs = previousBodyEndMs(lines, index);
  return lines[index].startMs - previousEndMs >= 5200 ? 'interlude' : null;
}

function previousBodyEndMs(lines, index) {
  const previous = lines[index - 1];
  if (!previous) {
    return 0;
  }
  return Number.isFinite(previous.endMs) ? previous.endMs : previous.startMs;
}
