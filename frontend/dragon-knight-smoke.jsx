import React from 'react';
import { createRoot } from 'react-dom/client';
import { LyricPlaybackView } from './src/LyricPlaybackView.jsx';
import { defaultLyricSettings, normalizeLyricPayload } from './src/lyricPlayback.js';
import payload from './dragon-knight-real.json';
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

const lyric = normalizeLyricPayload(payload);
window.__dragonKnightPayload = payload;
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
    settings={defaultLyricSettings}
    t={t}
  />,
);
