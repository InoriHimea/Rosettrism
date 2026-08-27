import React, { lazy, memo, Suspense, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Pause, Play, RotateCcw } from 'lucide-react';
import { extractAlbumColors } from './albumColor.js';
import { formatPlaybackTime, resolveLyricGradient } from './lyricPlayback.js';
import { createPreviewClock } from './playbackClock.js';
import {
  annotationAnchorPercent,
  annotationGlyphText,
  annotationLabelState,
  annotationLegendLabel,
  buildIntroMetaLines,
  buildPlaybackFrameState,
  displayTitleIncludesArtist,
  formatArtistLabel,
  lineAnnotationLabelIds,
  lineClassName,
  lyricLineProgress,
  lyricProgressStyle,
  looksLikeTitleDuplicate,
  normalizeMetaText,
  shouldReserveBreathGap,
  translationModeLabel,
  uniqueAnnotations,
  visibleWordAnnotations,
  wordProgress,
} from './lyricPlaybackViewModel.js';

const LazyLyricStage3D = lazy(() => import('./LyricStage3D.jsx').then((module) => ({
  default: module.LyricStage3D,
})));

export function LyricPlaybackView({ lyric, settings, t, clock: externalClock }) {
  const durationMs = Math.max(lyric.durationMs || 0, 1000);
  const clock = useMemo(
    () => externalClock || createPreviewClock({ durationMs }),
    [externalClock, durationMs],
  );
  const [playback, setPlayback] = useState(() => (
    clock.snapshot?.() || {
      currentMs: clock.nowMs(),
      durationMs: clock.durationMs(),
      isPlaying: clock.isPlaying(),
    }
  ));
  const [translationMode, setTranslationMode] = useState('off');
  const linesRef = useRef(null);
  const lineRefs = useRef(new Map());
  const currentMs = playback.currentMs;
  const effectiveDurationMs = playback.durationMs || durationMs;
  const isPlaying = playback.isPlaying;
  const isBuffering = Boolean(playback.isBuffering);
  const mediaError = playback.error || null;
  const playbackRate = Number(playback.playbackRate) || 1;
  const renderMode = settings.renderMode === 'vertical' ? 'vertical' : 'karaoke';
  const artworkUrl = settings.artwork || null;
  const [albumColors, setAlbumColors] = useState(null);

  useEffect(() => {
    if (settings.colorMode !== 'album' || !artworkUrl) {
      setAlbumColors(null);
      return undefined;
    }
    let cancelled = false;
    extractAlbumColors(artworkUrl)
      .then((colors) => {
        if (!cancelled) {
          setAlbumColors(colors);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setAlbumColors(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [settings.colorMode, artworkUrl]);

  const annotationTypes = useMemo(
    () => [...new Map(lyric.annotations.map((annotation) => [annotation.type, annotation])).values()],
    [lyric.annotations],
  );
  const headingTitle = lyric.displayTitle || lyric.title || t.preview;
  const artistLabel = displayTitleIncludesArtist(headingTitle, lyric.artist)
    ? ''
    : formatArtistLabel(lyric.artist, lyric.artistAlias);
  const metaLines = useMemo(() => lyric.lines.filter((line) => line.isMeta), [lyric.lines]);
  const bodyLines = useMemo(() => lyric.lines.filter((line) => !line.isMeta), [lyric.lines]);
  const firstBodyStartMs = bodyLines[0]?.startMs || 0;
  const introMetaLines = useMemo(
    () => buildIntroMetaLines(metaLines, firstBodyStartMs),
    [metaLines, firstBodyStartMs],
  );
  const introMetaEndMs = introMetaLines.length ? Math.max(...introMetaLines.map((line) => line.endMs || 0)) : 0;
  const flowLines = useMemo(() => [...introMetaLines, ...bodyLines], [introMetaLines, bodyLines]);
  const hasTranslations = bodyLines.some((line) => hasLineTranslation(line));
  const frameState = buildPlaybackFrameState({
    bodyLines,
    introMetaLines,
    currentMs,
    durationMs: effectiveDurationMs,
    introMetaEndMs,
  });
  const {
    activeBodyLine,
    activeMetaLine,
    activeFlowLine,
    countdown,
    focusBodyIndex,
    focusFlowIndex,
    nextBodyLine,
    nextFlowLine,
    showCountdown,
  } = frameState;
  const visibleFlowLine = activeFlowLine || nextFlowLine || flowLines[flowLines.length - 1];
  const visibleBodyLine = activeBodyLine || nextBodyLine || bodyLines[bodyLines.length - 1];
  const showStandaloneCountdown = showCountdown && !activeBodyLine;
  const currentStripText = renderMode === 'karaoke'
    ? ''
    : (showStandaloneCountdown ? '...' : activeFlowLine?.text || visibleBodyLine?.text);
  const initialFlowLine = flowLines[0];
  const countdownTargetId = showStandaloneCountdown && countdown.targetLineId ? `countdown-${countdown.targetLineId}` : null;
  const scrollTargetId = currentMs <= 0
    ? initialFlowLine?.id
    : (countdownTargetId || (activeFlowLine ? activeFlowLine.id : visibleFlowLine?.id));

  useEffect(() => {
    setPlayback(clock.snapshot?.() || {
      currentMs: clock.nowMs(),
      durationMs: clock.durationMs(),
      isPlaying: clock.isPlaying(),
    });
    const unsubscribe = clock.subscribe(setPlayback);
    return () => {
      unsubscribe();
      if (!externalClock) {
        clock.destroy();
      }
    };
  }, [clock, externalClock]);

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

  const syncClockSnapshot = useCallback(() => {
    setPlayback(clock.snapshot?.() || {
      currentMs: clock.nowMs(),
      durationMs: clock.durationMs(),
      isPlaying: clock.isPlaying(),
    });
  }, [clock]);

  const togglePlayback = useCallback(() => {
    if (clock.isPlaying()) {
      clock.pause();
      return;
    }
    Promise.resolve(clock.play()).catch(syncClockSnapshot);
  }, [clock, syncClockSnapshot]);

  const seek = useCallback((nextMs) => {
    clock.seek(nextMs);
  }, [clock]);

  const restart = useCallback(() => {
    clock.seek(0);
    Promise.resolve(clock.play()).catch(syncClockSnapshot);
  }, [clock, syncClockSnapshot]);

  const cyclePlaybackRate = useCallback(() => {
    const rates = [0.5, 1, 1.5, 2];
    const nextRate = rates[(rates.indexOf(playbackRate) + 1) % rates.length];
    if (typeof clock.setPlaybackRate === 'function') {
      clock.setPlaybackRate(nextRate);
      syncClockSnapshot();
    }
  }, [clock, playbackRate, syncClockSnapshot]);

  const cycleTranslationMode = useCallback(() => {
    setTranslationMode((mode) => (mode === 'off' ? 'translation' : mode === 'translation' ? 'bilingual' : 'off'));
  }, []);

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
  const ambientEffects = settings.ambientEffects === true;
  const stage3D = settings.stage3D === true;

  const style = {
    '--lyric-solid-color': settings.solidColor,
    '--lyric-gradient': resolveLyricGradient(settings.colorPreset),
    '--lyric-stage-background': settings.stageBackgroundColor || '#ffe58b',
    '--lyric-album-fill-start': albumColors?.start,
    '--lyric-album-fill-end': albumColors?.end,
  };

  return (
    <section
      className={`lyric-playback-card lyric-color-${settings.colorMode}`}
      style={style}
      data-playback-phase={frameState.phase}
      data-playback-time-ms={Math.round(currentMs)}
      data-media-buffering={isBuffering ? 'true' : 'false'}
      data-media-rate={String(playbackRate)}
    >
      <PlaybackHeader
        headingTitle={headingTitle}
        artistLabel={artistLabel}
        source={lyric.source}
        inputFormat={lyric.inputFormat}
        hasAnnotations={lyric.annotations.length > 0}
        quality={lyric.quality}
        t={t}
      />
      <AnnotationLegend annotationTypes={annotationTypes} t={t} />
      {isBuffering || mediaError ? (
        <div className={`lyric-media-notice${mediaError ? ' lyric-media-error' : ''}`} role={mediaError ? 'alert' : 'status'}>
          {mediaError || t.mediaBuffering || '正在缓冲音频…'}
        </div>
      ) : null}

      <div className={`lyric-stage lyric-stage-qq lyric-stage-${renderMode} lyric-motion-${motionPreset}${lowDistraction ? ' lyric-low-distraction' : ''}${ambientEffects ? ' lyric-ambient-on' : ' lyric-ambient-off'}${stage3D ? ' lyric-stage-3d-on' : ''}`} data-testid="karaoke-stage">
        {stage3D ? (
          <Suspense fallback={null}>
            <LazyLyricStage3D
              currentMs={currentMs}
              durationMs={effectiveDurationMs}
              isPlaying={isPlaying}
              phase={frameState.phase}
              colorPreset={settings.colorPreset}
              colorMode={settings.colorMode}
              solidColor={settings.solidColor}
              lowDistraction={lowDistraction}
              stageBackgroundColor={settings.stageBackgroundColor}
            />
          </Suspense>
        ) : (ambientEffects ? <LyricAmbientEffects /> : null)}
        {renderMode === 'karaoke' ? (
          <KaraokeStage
            laneItems={frameState.laneItems}
            headingTitle={headingTitle}
            currentMs={currentMs}
            activeBodyLine={activeBodyLine}
            introMetaLines={introMetaLines}
            introMetaEndMs={introMetaEndMs}
            focusBodyIndex={focusBodyIndex}
            showCountdown={showStandaloneCountdown}
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
              const countdownBeforeLine = showStandaloneCountdown && countdown.targetLineId === line.id;
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
        {renderMode !== 'karaoke' ? (
          <div className={`lyric-current-strip${countdown.flashing ? ' lyric-dots-flashing' : ''}${countdown.exiting ? ' lyric-dots-exiting' : ''}`} aria-live="polite">
            {currentStripText || '...'}
          </div>
        ) : null}
      </div>

      <div className="lyric-controls">
        <PlaybackTimeline
          currentMs={currentMs}
          durationMs={effectiveDurationMs}
          annotations={lyric.annotations}
          onSeek={seek}
          t={t}
        />
        <PlaybackActions
          isPlaying={isPlaying}
          translationMode={translationMode}
          hasTranslations={hasTranslations}
          onTogglePlayback={togglePlayback}
          onRestart={restart}
          onCycleTranslation={cycleTranslationMode}
          playbackRate={playbackRate}
          canChangePlaybackRate={typeof clock.setPlaybackRate === 'function'}
          onCyclePlaybackRate={cyclePlaybackRate}
          t={t}
        />
      </div>
    </section>
  );
}

const PlaybackActions = memo(function PlaybackActions({
  isPlaying,
  translationMode,
  hasTranslations,
  onTogglePlayback,
  onRestart,
  onCycleTranslation,
  playbackRate,
  canChangePlaybackRate,
  onCyclePlaybackRate,
  t,
}) {
  return (
    <div className="lyric-playback-actions">
      <button className="button-primary lyric-control-button lyric-play-toggle" type="button" onClick={onTogglePlayback}>
        {isPlaying ? <Pause size={19} /> : <Play size={19} />}
        {isPlaying ? t.pause : t.play}
      </button>
      <button className="button-secondary lyric-control-button lyric-secondary-action" type="button" onClick={onRestart}>
        <RotateCcw size={17} />
        {t.restart}
      </button>
      <button
        className={`button-secondary lyric-control-button lyric-secondary-action lyric-translation-toggle${translationMode !== 'off' ? ' active' : ''}`}
        type="button"
        onClick={onCycleTranslation}
        disabled={!hasTranslations}
        aria-pressed={translationMode !== 'off'}
      >
        {translationModeLabel(translationMode, t)}
      </button>
      {canChangePlaybackRate ? (
        <button
          className="button-secondary lyric-control-button lyric-secondary-action lyric-rate-toggle"
          type="button"
          onClick={onCyclePlaybackRate}
          aria-label={t.playbackRate || '播放速度'}
        >
          {playbackRate}x
        </button>
      ) : null}
    </div>
  );
});

function PlaybackTimeline({ currentMs, durationMs, annotations, onSeek, t }) {
  return (
    <div className="lyric-timeline-control">
      <div className="lyric-time">
        <span>{formatPlaybackTime(currentMs)}</span>
        <span aria-hidden="true">/</span>
        <span>{formatPlaybackTime(durationMs)}</span>
      </div>
      <label className="lyric-seek">
        <span>{t.timeline}</span>
        <input
          type="range"
          min="0"
          max={durationMs}
          value={Math.round(currentMs)}
          onChange={(event) => onSeek(event.target.value)}
          aria-label={t.timeline}
          aria-valuetext={formatPlaybackTime(currentMs)}
        />
        {annotations.length > 0 ? (
          <span className="lyric-timeline" aria-hidden="true">
            {annotations.map((annotation) => (
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
  );
}

const PlaybackHeader = memo(function PlaybackHeader({
  headingTitle,
  artistLabel,
  source,
  inputFormat,
  hasAnnotations,
  quality,
  t,
}) {
  return (
    <div className="lyric-playback-head">
      <div>
        <span>{t.playback}</span>
        <h4>{headingTitle}</h4>
        <p>{[artistLabel, source, inputFormat].filter(Boolean).join(' / ')}</p>
      </div>
      <div className="lyric-playback-statuses">
        <span
          className={`lyric-playback-status lyric-quality-status lyric-quality-${quality?.timingLevel || 'unknown'}`}
          title={qualityDiagnosticSummary(quality, t)}
          data-testid="lyric-quality-status"
        >
          {qualityLabel(quality, t)}
        </span>
        <span className={`lyric-playback-status ${hasAnnotations ? 'status-fresh' : ''}`}>
          {hasAnnotations ? t.annotationsAvailable : t.annotationsUnavailable}
        </span>
      </div>
    </div>
  );
});

function qualityLabel(quality, t) {
  const labels = {
    word_timed: t.lyricQualityWordTimed || '逐字同步',
    line_timed: t.lyricQualityLineTimed || '逐行同步',
    unsynced: t.lyricQualityUnsynced || '无同步时间',
    invalid: t.lyricQualityInvalid || '时间轴异常',
  };
  return labels[quality?.timingLevel] || t.lyricQualityUnknown || '质量未评估';
}

function qualityDiagnosticSummary(quality, t) {
  const count = quality?.diagnostics?.filter(
    (item) => item.severity !== 'info',
  ).length || 0;
  if (!count) {
    return t.lyricQualityHealthy || '歌词时间轴通过质量检查';
  }
  return (t.lyricQualityIssueCount || '{count} 个时间轴问题')
    .replace('{count}', String(count));
}

const AnnotationLegend = memo(function AnnotationLegend({ annotationTypes, t }) {
  if (!annotationTypes.length) {
    return null;
  }
  return (
    <div className="lyric-annotation-legend" aria-label={t.annotations}>
      {annotationTypes.map((annotation) => (
        <span className={`lyric-annotation-chip ${annotation.className}`} key={annotation.type}>
          <AnnotationGlyph type={annotation.type} />
          <span>{annotationLegendLabel(annotation, t)}</span>
        </span>
      ))}
    </div>
  );
});

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
    <span className="lyric-gap-dots" aria-label="countdown bubbles">
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
  laneItems,
  headingTitle,
  currentMs,
  activeBodyLine,
  introMetaLines,
  introMetaEndMs,
  focusBodyIndex,
  showCountdown,
  countdown,
  translationMode,
  seek,
  bindLineRef,
  linesRef,
  t,
}) {
  return (
    <div className="lyric-karaoke-lines lyric-karaoke-dual-lines" ref={linesRef} aria-live="polite">
      <KaraokeMetaPanel
        lines={introMetaLines}
        currentMs={currentMs}
        headingTitle={headingTitle}
        showCountdown={showCountdown}
        countdown={countdown}
        introMetaEndMs={introMetaEndMs}
      />
      {laneItems.map((item) => (
        <KaraokeLane
          item={item}
          currentMs={currentMs}
          activeBodyLine={activeBodyLine}
          focusBodyIndex={focusBodyIndex}
          translationMode={translationMode}
          seek={seek}
          bindLineRef={bindLineRef}
          t={t}
          key={item.key}
        />
      ))}
    </div>
  );
}

function KaraokeLane({
  item,
  currentMs,
  activeBodyLine,
  focusBodyIndex,
  translationMode,
  seek,
  bindLineRef,
  t,
}) {
  const lineNodeRef = useRef(null);
  const [fit, setFit] = useState('normal');
  const line = item.line;

  useLayoutEffect(() => {
    const node = lineNodeRef.current;
    if (!node || item.kind === 'empty') {
      setFit('normal');
      return undefined;
    }

    const measure = () => {
      const nextFit = measureLyricLineFit(node);
      setFit((current) => current === nextFit ? current : nextFit);
    };
    measure();
    window.addEventListener('resize', measure);
    return () => window.removeEventListener('resize', measure);
  }, [item.kind, line?.id, translationMode]);

  const laneClassName = `lyric-karaoke-lane ${item.lanePositionClass} lyric-karaoke-role-${item.role} lyric-karaoke-transition-${item.transition}`;
  if (item.kind === 'empty') {
    return (
      <span
        className={`${laneClassName} lyric-karaoke-placeholder`}
        data-lane-slot={item.slot}
        data-lane-role={item.role}
        aria-hidden="true"
      />
    );
  }
  if (item.kind === 'countdown') {
    return (
      <div className={laneClassName} data-lane-slot={item.slot} data-lane-role={item.role}>
        <CountdownRow
          countdown={item.countdown}
          laneClass={`${item.laneClass} lyric-karaoke-line ${item.lanePositionClass}`}
          refCallback={bindLineRef(`countdown-${item.targetLine?.id || item.slot}`)}
        />
      </div>
    );
  }

  const { bodyIndex } = item;
  const isActive = activeBodyLine?.id === line.id;
  const lineRef = (node) => {
    lineNodeRef.current = node;
    bindLineRef(line.id)(node);
  };
  return (
    <div
      className={laneClassName}
      data-lane-slot={item.slot}
      data-lane-role={item.role}
      data-lane-line-id={line.id}
    >
      <button
        className={`${lineClassName(line, currentMs, isActive, bodyIndex, focusBodyIndex)} lyric-karaoke-line ${item.laneClass}`}
        data-fit={fit}
        type="button"
        onClick={() => seek(line.startMs)}
        ref={lineRef}
      >
        <LineText line={line} currentMs={currentMs} active={isActive} translationMode={translationMode} t={t} />
        <LineSubtext line={line} translationMode={translationMode} />
      </button>
    </div>
  );
}

function measureLyricLineFit(node) {
  const levels = ['normal', 'compact', 'tight', 'wrap'];
  for (const level of levels) {
    node.dataset.fit = level;
    const content = node.querySelector('.lyric-words, .lyric-line-text');
    const laneWidth = Math.max(1, node.parentElement?.clientWidth || node.clientWidth);
    const maxWidth = Number.parseFloat(globalThis.getComputedStyle?.(node).maxWidth);
    const availableWidth = Number.isFinite(maxWidth) ? Math.min(laneWidth, maxWidth) : laneWidth;
    const contentWidth = Math.max(content?.scrollWidth || 0, node.scrollWidth);
    if (contentWidth <= availableWidth + 1 || level === 'wrap') {
      return level;
    }
  }
  return 'wrap';
}

function KaraokeMetaPanel({ lines, currentMs, headingTitle, showCountdown, countdown, introMetaEndMs }) {
  const titleText = String(headingTitle || '').trim();
  if (!titleText && !lines.length) {
    return null;
  }
  const normalizedTitle = normalizeMetaText(titleText);
  const detailLines = lines.filter((line) => {
    const text = String(line.text || '').trim();
    return text && normalizeMetaText(text) !== normalizedTitle && !looksLikeTitleDuplicate(text, titleText);
  });
  const detailText = detailLines.map((line) => String(line.text || '').trim()).filter(Boolean).join(' / ');
  const showDetail = Boolean(detailText && currentMs < introMetaEndMs);
  const showBubble = Boolean(showCountdown && currentMs >= introMetaEndMs);
  return (
    <div className="lyric-karaoke-meta-panel" aria-live="polite">
      {showDetail ? (
        <span className="lyric-karaoke-meta-detail">
          <span className="lyric-karaoke-meta-line">{detailText}</span>
        </span>
      ) : showBubble ? (
        <span className={`lyric-karaoke-meta-detail lyric-karaoke-meta-countdown${countdown?.flashing ? ' lyric-dots-flashing' : ''}${countdown?.exiting ? ' lyric-dots-exiting' : ''}`}>
          <CountdownDots count={countdown?.count || 0} exiting={Boolean(countdown?.exiting)} />
        </span>
      ) : null}
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
  const visibleLabelIds = active ? lineAnnotationLabelIds(line.words, orphanAnnotations) : new Set();

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
            style={lyricProgressStyle(progress)}
            key={word.id}
          >
            {annotations.length > 0 ? <AnnotationLayer annotations={annotations} active={active} visibleLabelIds={visibleLabelIds} t={t} /> : null}
            <span className="lyric-progress-base">{word.text}</span>
            <span className="lyric-progress-fill" aria-hidden="true">{word.text}</span>
          </span>
        );
      })}
      {orphanAnnotations.length > 0 ? (
        <span className="lyric-word lyric-word-orphan" aria-hidden="true">
          <AnnotationLayer annotations={orphanAnnotations} active={active} visibleLabelIds={visibleLabelIds} t={t} />
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

function LineSubtext({ line, translationMode }) {
  const translation = line.translation || line.englishTranslation || '';
  const reading = line.reading || line.romanized || '';
  const showTranslation = translationMode === 'bilingual' && Boolean(translation);
  const showReading = Boolean(reading);
  return (
    <span
      className={`lyric-line-subtext${showTranslation || showReading ? ' lyric-line-subtext-visible' : ''}`}
      aria-hidden={!showTranslation && !showReading}
    >
      <small className="lyric-line-translation">{showTranslation ? translation : '\u00a0'}</small>
      <small className="lyric-line-reading">{showReading ? reading : '\u00a0'}</small>
    </span>
  );
}

function hasLineTranslation(line) {
  return Boolean(line.translation || line.englishTranslation);
}

function AnnotationLayer({ annotations, active, visibleLabelIds, t }) {
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
        const showLabel = active && (visibleLabelIds ? visibleLabelIds.has(annotation.id) : labelState.ids.has(annotation.id));
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
