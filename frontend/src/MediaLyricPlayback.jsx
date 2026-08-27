import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ListEnd,
  ListRestart,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { createMediaClock } from './playbackClock.js';
import {
  createPlayerSessionState,
  normalizeQueue,
  persistPlayerSession,
  PLAYBACK_MODES,
  readPersistedPlayerSession,
  reconcilePlayerSession,
  reducePlayerSession,
} from './playerSession.js';
import { LyricPlaybackView } from './LyricPlaybackView.jsx';

const MODE_ICONS = {
  order: ListEnd,
  'repeat-one': Repeat1,
  'repeat-all': ListRestart,
  shuffle: Shuffle,
};

const MODE_LABEL_KEYS = {
  order: 'playbackMode_order',
  'repeat-one': 'playbackMode_repeatOne',
  'repeat-all': 'playbackMode_repeatAll',
  shuffle: 'playbackMode_shuffle',
};

export function MediaLyricPlayback({
  lyric,
  settings,
  t,
  audioSrc,
  queue,
  initialTrackId,
  persistSession = false,
  onReady,
  onSessionChange,
}) {
  const inputQueue = useMemo(() => {
    const normalized = normalizeQueue(queue);
    if (normalized.length) {
      return normalized;
    }
    return audioSrc ? normalizeQueue([{
      id: 'current-track',
      src: audioSrc,
      title: lyric?.displayTitle || lyric?.title || t.preview,
      artist: lyric?.artist || '',
      lyric,
      durable: false,
    }]) : [];
  }, [audioSrc, lyric, queue, t.preview]);
  const [session, setSession] = useState(() => {
    const restored = persistSession ? readPersistedPlayerSession() : createPlayerSessionState();
    return reconcilePlayerSession(restored, inputQueue);
  });
  const sessionRef = useRef(session);
  const mediaRef = useRef(null);
  const restorePositionRef = useRef(session.positionMs);
  const autoplayRef = useRef(false);
  const mediaReadyRef = useRef(false);
  const [media, setMedia] = useState(null);
  const clock = useMemo(() => (media ? createMediaClock(media) : null), [media]);
  const currentTrack = session.queue[session.currentIndex] || null;
  const currentLyric = currentTrack?.lyric || lyric;

  useEffect(() => {
    sessionRef.current = session;
    onSessionChange?.(session);
  }, [onSessionChange, session]);

  useEffect(() => {
    const node = mediaRef.current;
    if (node) {
      setMedia(node);
    }
  }, []);

  useEffect(() => {
    setSession((current) => {
      const reconciled = reconcilePlayerSession(current, inputQueue);
      if (initialTrackId && reconciled.queue.some((item) => item.id === initialTrackId)) {
        return reducePlayerSession(reconciled, { type: 'select', id: initialTrackId, autoplay: false });
      }
      return reconciled;
    });
  }, [initialTrackId, inputQueue]);

  useEffect(() => {
    if (!media || !currentTrack) {
      return;
    }
    mediaReadyRef.current = false;
    media.pause();
    media.src = currentTrack.src;
    media.load();
  }, [currentTrack?.id, currentTrack?.src, media]);

  useEffect(() => {
    if (!media || !clock) {
      return undefined;
    }
    const handleLoadedMetadata = () => {
      const current = sessionRef.current;
      clock.setVolume?.(current.volume);
      clock.setMuted?.(current.muted);
      clock.setPlaybackRate?.(current.playbackRate);
      mediaReadyRef.current = true;
      const restorePosition = restorePositionRef.current;
      restorePositionRef.current = 0;
      if (restorePosition > 0) {
        clock.seek(Math.min(restorePosition, clock.durationMs()));
      }
      if (autoplayRef.current) {
        autoplayRef.current = false;
        Promise.resolve(clock.play()).catch((error) => {
          setSession((current) => reducePlayerSession(current, { type: 'media-error', error: error?.message }));
        });
      }
    };
    const handleEnded = () => {
      setSession((current) => {
        const next = reducePlayerSession(current, { type: 'ended' });
        autoplayRef.current = next.isPlaying;
        if (next.currentId === current.currentId && next.isPlaying) {
          clock.seek(0);
          Promise.resolve(clock.play()).catch(() => {});
        }
        return next;
      });
    };
    const handleError = () => {
      const error = clock.snapshot().error;
      if (error) {
        setSession((current) => reducePlayerSession(current, { type: 'media-error', error }));
      }
    };
    const handlePlay = () => setSession((current) => reducePlayerSession(current, { type: 'set-playing', isPlaying: true }));
    const handlePause = () => {
      if (!media.ended) {
        setSession((current) => reducePlayerSession(current, { type: 'set-playing', isPlaying: false }));
      }
    };
    media.addEventListener('loadedmetadata', handleLoadedMetadata);
    media.addEventListener('ended', handleEnded);
    media.addEventListener('error', handleError);
    media.addEventListener('play', handlePlay);
    media.addEventListener('pause', handlePause);
    return () => {
      media.removeEventListener('loadedmetadata', handleLoadedMetadata);
      media.removeEventListener('ended', handleEnded);
      media.removeEventListener('error', handleError);
      media.removeEventListener('play', handlePlay);
      media.removeEventListener('pause', handlePause);
    };
  }, [clock, media]);

  useEffect(() => {
    if (!media || !clock) {
      return undefined;
    }
    let lastSignature = '';
    return clock.subscribe((snapshot) => {
      if (!mediaReadyRef.current) {
        return;
      }
      const signature = [
        Math.floor(snapshot.currentMs / 1000),
        snapshot.volume,
        snapshot.muted,
        snapshot.playbackRate,
      ].join(':');
      if (signature === lastSignature) {
        return;
      }
      lastSignature = signature;
      setSession((current) => {
        const next = {
          ...current,
          positionMs: snapshot.currentMs,
          volume: snapshot.volume ?? current.volume,
          muted: snapshot.muted ?? current.muted,
          playbackRate: snapshot.playbackRate ?? current.playbackRate,
        };
        sessionRef.current = next;
        if (persistSession) {
          persistPlayerSession(next);
        }
        return next;
      });
    });
  }, [clock, media, persistSession]);

  useEffect(() => {
    if (persistSession) {
      persistPlayerSession(session);
    }
  }, [persistSession, session]);

  const selectTrack = useCallback((id, autoplay = false) => {
    setSession((current) => {
      autoplayRef.current = autoplay;
      restorePositionRef.current = 0;
      return reducePlayerSession(current, { type: 'select', id, autoplay });
    });
  }, []);

  const moveTrack = useCallback((type) => {
    setSession((current) => {
      const next = reducePlayerSession(current, { type, autoplay: true });
      autoplayRef.current = next.isPlaying;
      restorePositionRef.current = 0;
      return next;
    });
  }, []);

  const setMode = useCallback((mode) => {
    setSession((current) => reducePlayerSession(current, { type: 'set-mode', mode }));
  }, []);

  const seek = useCallback((positionMs) => {
    const numeric = Math.max(0, Number(positionMs) || 0);
    restorePositionRef.current = numeric;
    clock?.seek(numeric);
    setSession((current) => {
      const next = reducePlayerSession(current, { type: 'set-position', positionMs: numeric });
      sessionRef.current = next;
      if (persistSession) {
        persistPlayerSession(next);
      }
      return next;
    });
  }, [clock, persistSession]);

  const setVolume = useCallback((volume) => {
    clock?.setVolume?.(volume);
    setSession((current) => reducePlayerSession(current, { type: 'set-volume', volume }));
  }, [clock]);

  const toggleMuted = useCallback(() => {
    setSession((current) => {
      const muted = !current.muted;
      clock?.setMuted?.(muted);
      return reducePlayerSession(current, { type: 'set-muted', muted });
    });
  }, [clock]);

  const retry = useCallback(() => {
    if (!media || !currentTrack) {
      return;
    }
    setSession((current) => reducePlayerSession(current, { type: 'retry' }));
    autoplayRef.current = false;
    restorePositionRef.current = sessionRef.current.positionMs;
    media.load();
  }, [currentTrack, media]);

  useEffect(() => {
    if (!media || !clock || !('mediaSession' in navigator)) {
      return undefined;
    }
    const mediaSession = navigator.mediaSession;
    if (globalThis.MediaMetadata && currentTrack) {
      mediaSession.metadata = new MediaMetadata({
        title: currentTrack.title,
        artist: currentTrack.artist,
        album: currentTrack.album,
        artwork: currentTrack.artwork ? [{ src: currentTrack.artwork }] : [],
      });
    }
    const handlers = {
      play: () => clock.play(),
      pause: () => clock.pause(),
      previoustrack: () => moveTrack('previous'),
      nexttrack: () => moveTrack('next'),
      seekbackward: (details) => clock.seek(clock.nowMs() - (details.seekOffset || 10) * 1000),
      seekforward: (details) => clock.seek(clock.nowMs() + (details.seekOffset || 10) * 1000),
      seekto: (details) => clock.seek((details.seekTime || 0) * 1000),
    };
    for (const [action, handler] of Object.entries(handlers)) {
      try {
        mediaSession.setActionHandler(action, handler);
      } catch {
        // Some browsers expose Media Session with only a subset of actions.
      }
    }
    return () => {
      for (const action of Object.keys(handlers)) {
        try {
          mediaSession.setActionHandler(action, null);
        } catch {
          // Ignore unsupported action cleanup.
        }
      }
    };
  }, [clock, currentTrack, media, moveTrack]);

  useEffect(() => {
    if (!media || !clock) {
      return undefined;
    }
    const controller = {
      media,
      clock,
      seek,
      session: () => sessionRef.current,
      selectTrack,
      next: () => moveTrack('next'),
      previous: () => moveTrack('previous'),
      setMode,
      setVolume,
      toggleMuted,
      retry,
    };
    onReady?.(controller);
    return () => onReady?.(null);
  }, [clock, media, moveTrack, onReady, retry, seek, selectTrack, setMode, setVolume, toggleMuted]);

  useEffect(() => () => {
    clock?.destroy?.();
  }, [clock]);

  if (!currentTrack || !currentLyric) {
    return <p className="lyric-media-notice" role="status">{t.mediaSourceMissing || '缺少合法音频源'}</p>;
  }

  return (
    <div className="media-playback-harness" data-testid="media-playback-harness">
      <audio ref={mediaRef} className="media-playback-audio" preload="auto" aria-hidden="true" />
      <PlayerSessionControls
        session={session}
        onSelect={(id) => selectTrack(id, true)}
        onPrevious={() => moveTrack('previous')}
        onNext={() => moveTrack('next')}
        onMode={setMode}
        onVolume={setVolume}
        onToggleMuted={toggleMuted}
        onRetry={retry}
        t={t}
      />
      {clock ? (
        <LyricPlaybackView lyric={currentLyric} settings={currentTrack?.artwork ? { ...settings, artwork: currentTrack.artwork } : settings} t={t} clock={clock} />
      ) : (
        <p className="lyric-media-notice" role="status">{t.mediaInitializing || '正在初始化音频…'}</p>
      )}
    </div>
  );
}

function PlayerSessionControls({
  session,
  onSelect,
  onPrevious,
  onNext,
  onMode,
  onVolume,
  onToggleMuted,
  onRetry,
  t,
}) {
  const ModeIcon = MODE_ICONS[session.mode] || ListEnd;
  const modeIndex = PLAYBACK_MODES.indexOf(session.mode);
  const nextMode = PLAYBACK_MODES[(modeIndex + 1) % PLAYBACK_MODES.length];
  const modeLabel = t[MODE_LABEL_KEYS[session.mode]] || session.mode;
  const currentTrack = session.queue[session.currentIndex];

  return (
    <div className="player-session-bar" data-testid="player-session-bar" data-playback-mode={session.mode}>
      <div className="player-session-track">
        <strong>{currentTrack?.title}</strong>
        {currentTrack?.artist ? <span>{currentTrack.artist}</span> : null}
      </div>
      <div className="player-session-actions">
        <button className="button-icon player-session-icon" type="button" onClick={onPrevious} title={t.previousTrack} aria-label={t.previousTrack}>
          <SkipBack size={18} />
        </button>
        <button className="button-icon player-session-icon" type="button" onClick={onNext} title={t.nextTrack} aria-label={t.nextTrack}>
          <SkipForward size={18} />
        </button>
        <button className="button-icon player-session-icon" type="button" onClick={() => onMode(nextMode)} title={modeLabel} aria-label={`${t.playbackMode}: ${modeLabel}`}>
          <ModeIcon size={18} />
        </button>
        <label className="player-session-queue">
          <span>{t.playQueue}</span>
          <select value={session.currentId || ''} onChange={(event) => onSelect(event.target.value)} aria-label={t.playQueue}>
            {session.queue.map((item) => <option value={item.id} key={item.id}>{item.title}{item.artist ? ` · ${item.artist}` : ''}</option>)}
          </select>
        </label>
        <label className="player-session-volume">
          <span>{t.volume}</span>
          <input type="range" min="0" max="1" step="0.05" value={session.volume} onChange={(event) => onVolume(event.target.value)} aria-label={t.volume} />
        </label>
        <button className="button-icon player-session-icon" type="button" onClick={onToggleMuted} title={session.muted ? t.unmute : t.mute} aria-label={session.muted ? t.unmute : t.mute} aria-pressed={session.muted}>
          {session.muted ? <VolumeX size={18} /> : <Volume2 size={18} />}
        </button>
        {session.error ? (
          <button className="button-secondary player-session-retry" type="button" onClick={onRetry}>{t.retryAudio}</button>
        ) : null}
      </div>
    </div>
  );
}
