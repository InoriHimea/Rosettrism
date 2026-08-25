import React, { useCallback, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { dictionaries } from './i18n/dictionaries.js';
import { defaultLyricSettings, normalizeLyricPayload } from './lyricPlayback.js';
import { MediaLyricPlayback } from './MediaLyricPlayback.jsx';
import './styles.css';

const config = globalThis.__LYRIC_MEDIA_HARNESS__ || {};
const lyric = normalizeLyricPayload(config.payload || {});
const settings = { ...defaultLyricSettings, ...(config.settings || {}) };

function MediaPlaybackHarness() {
  const queue = useMemo(() => {
    if (Array.isArray(config.queue)) {
      return config.queue.filter(Boolean).map((item) => ({ ...item, lyric: item.lyric ? normalizeLyricPayload(item.lyric) : lyric }));
    }
    const sources = Array.isArray(config.sources) ? config.sources.filter(Boolean) : [config.audioSrc].filter(Boolean);
    return sources.map((src, index) => ({
      id: `source-${index}`,
      src,
      title: `测试音轨 ${index + 1}`,
      artist: 'Rosettrism',
      lyric,
      durable: Boolean(config.durable),
    }));
  }, []);
  const [controller, setController] = useState(null);
  const [sessionState, setSessionState] = useState(null);
  const handleReady = useCallback((value) => {
    setController(value);
    globalThis.__LYRIC_MEDIA_HARNESS_API__ = value ? {
      currentMs: () => value.clock.nowMs(),
      durationMs: () => value.clock.durationMs(),
      mediaState: () => value.clock.snapshot(),
      sessionState: () => value.session(),
      pause: () => value.clock.pause(),
      play: () => value.clock.play(),
      seek: value.seek,
      setPlaybackRate: (rate) => value.clock.setPlaybackRate(rate),
      setVolume: value.setVolume,
      toggleMuted: value.toggleMuted,
      next: value.next,
      previous: value.previous,
      setMode: value.setMode,
      retry: value.retry,
      selectTrack: value.selectTrack,
      switchSource: (index) => value.selectTrack(queue[Math.max(0, Math.min(queue.length - 1, Number(index) || 0))]?.id, false),
    } : null;
  }, [queue]);

  if (!lyric.playable) {
    return (
      <main className="playback-harness-shell">
        <section className="playback-harness-fallback" data-testid="playback-fallback">
          <strong>暂无可播放时间轴</strong>
        </section>
      </main>
    );
  }

  if (!queue.length) {
    return (
      <main className="playback-harness-shell">
        <section className="playback-harness-fallback" data-testid="media-source-fallback">
          <strong>缺少合法音频源</strong>
        </section>
      </main>
    );
  }

  return (
    <main className="playback-harness-shell">
      <MediaLyricPlayback
        lyric={lyric}
        settings={settings}
        t={dictionaries.zh}
        queue={queue}
        persistSession={Boolean(config.persistSession)}
        onReady={handleReady}
        onSessionChange={setSessionState}
      />
      <output className="media-harness-source" data-testid="media-source-index">
        {controller && sessionState ? sessionState.currentIndex : 'loading'}
      </output>
    </main>
  );
}

createRoot(document.getElementById('root')).render(<MediaPlaybackHarness />);
