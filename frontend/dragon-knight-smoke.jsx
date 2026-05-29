import React from 'react';
import { createRoot } from 'react-dom/client';
import { LyricPlaybackView } from './src/LyricPlaybackView.jsx';
import { defaultLyricSettings, normalizeLyricPayload } from './src/lyricPlayback.js';
import './src/styles.css';

const t = {
  preview: '内容预览',
  playback: '歌词播放',
  annotationsAvailable: '已加载助唱标注',
  annotationsUnavailable: '无助唱标注',
  annotations: '助唱标注',
  annotationStress: '重音',
  annotationBreath: '换气',
  annotationLongTone: '长音',
  annotationPortamentoUp: '上滑音',
  annotationPortamentoDown: '下滑音',
  pause: '暂停',
  play: '播放',
  restart: '重新播放',
  timeline: '时间轴',
  lyricTranslationOff: '原文',
  lyricTranslationOnly: '译文',
  lyricTranslationBilingual: '双语',
};

const payload = {
  document: {
    meta: { source: 'QQ音乐', input_format: 'qrc' },
    lines: [
      {
        start_ms: 0,
        duration_ms: 20098,
        text: '龙战骑士 - 周杰伦 (Jay Chou)',
        words: [
          { offset_ms: 0, duration_ms: 1827, text: '龙' },
          { offset_ms: 1827, duration_ms: 1827, text: '战' },
          { offset_ms: 3654, duration_ms: 1827, text: '骑' },
          { offset_ms: 5482, duration_ms: 1827, text: '士' },
          { offset_ms: 7309, duration_ms: 1827, text: ' - ' },
          { offset_ms: 9136, duration_ms: 1827, text: '周' },
          { offset_ms: 10963, duration_ms: 1827, text: '杰' },
          { offset_ms: 12790, duration_ms: 1827, text: '伦' },
          { offset_ms: 14617, duration_ms: 1827, text: ' (' },
          { offset_ms: 16445, duration_ms: 1827, text: 'Jay ' },
          { offset_ms: 18272, duration_ms: 1827, text: 'Chou)' },
        ],
      },
      {
        start_ms: 16346,
        duration_ms: 3408,
        text: '久晴天',
        ruby: [{ startChar: 0, endChar: 1, reading: 'jiu' }],
        words: [
          { offset_ms: 0, duration_ms: 349, text: '久' },
          { offset_ms: 1243, duration_ms: 548, text: '晴' },
          { offset_ms: 1791, duration_ms: 346, text: '天' },
        ],
      },
      {
        start_ms: 21000,
        duration_ms: 2400,
        text: '逐字推进',
        words: [
          { offset_ms: 0, duration_ms: 500, text: '逐' },
          { offset_ms: 500, duration_ms: 500, text: '字' },
          { offset_ms: 1000, duration_ms: 500, text: '推' },
          { offset_ms: 1500, duration_ms: 500, text: '进' },
        ],
      },
    ],
  },
  singing_annotations: [
    { annotation_type: 'breath', start_ms: 16346, duration_ms: 349, text: '久' },
    { annotation_type: 'stress', start_ms: 17589, duration_ms: 548, text: '晴' },
    { annotation_type: 'long_tone', start_ms: 18137, duration_ms: 346, text: '天' },
  ],
};

const lyric = normalizeLyricPayload(payload);
window.__dragonKnightLyric = lyric;
window.__setDragonKnightTime = (value) => {
  const input = document.querySelector('.lyric-seek input');
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
  setter.call(input, String(value));
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
};

createRoot(document.getElementById('root')).render(
  <LyricPlaybackView
    lyric={lyric}
    settings={{ ...defaultLyricSettings, renderMode: 'karaoke' }}
    t={t}
  />,
);
