import React, { Profiler } from 'react';
import { createRoot } from 'react-dom/client';
import { dictionaries } from './i18n/dictionaries.js';
import { defaultLyricSettings, normalizeLyricPayload } from './lyricPlayback.js';
import { LyricPlaybackView } from './LyricPlaybackView.jsx';
import { createFakeClock } from './playbackClock.js';
import './styles.css';

const config = globalThis.__LYRIC_PLAYBACK_HARNESS__ || {};
const lyric = normalizeLyricPayload(config.payload || {});
const durationMs = Math.max(lyric.durationMs || 0, Number(config.durationMs) || 0, 1000);
const clock = createFakeClock({ durationMs, initialMs: Number(config.initialMs) || 0 });
const settings = { ...defaultLyricSettings, ...(config.settings || {}) };
const commits = [];

function onRender(id, phase, actualDuration, baseDuration, startTime, commitTime) {
  commits.push({ id, phase, actualDuration, baseDuration, startTime, commitTime });
}

globalThis.__LYRIC_PLAYBACK_HARNESS_API__ = {
  advance(deltaMs) {
    clock.advance(deltaMs);
  },
  clearMetrics() {
    commits.length = 0;
  },
  metrics() {
    return {
      commits: [...commits],
      currentMs: clock.nowMs(),
      durationMs: clock.durationMs(),
      isPlaying: clock.isPlaying(),
    };
  },
  pause() {
    clock.pause();
  },
  play() {
    clock.play();
  },
  seek(ms) {
    clock.seek(ms);
  },
};

createRoot(document.getElementById('root')).render(
  <main className="playback-harness-shell">
    {lyric.playable ? (
      <Profiler id="LyricPlaybackView" onRender={onRender}>
        <LyricPlaybackView
          lyric={lyric}
          settings={settings}
          t={dictionaries.zh}
          clock={clock}
        />
      </Profiler>
    ) : (
      <section className="playback-harness-fallback" data-testid="playback-fallback">
        <strong>暂无可播放时间轴</strong>
        <p>{lyric.raw || '当前歌词不包含可用的时间信息。'}</p>
      </section>
    )}
  </main>,
);
