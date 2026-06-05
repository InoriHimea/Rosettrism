import { useEffect, useState } from 'react';
import { readLyricSettings } from '../lyricPlayback.js';

export function useLyricSettings() {
  const [lyricSettings, setLyricSettings] = useState(readLyricSettings);

  useEffect(() => {
    localStorage.setItem('rosettrism-lyric-settings', JSON.stringify(lyricSettings));
  }, [lyricSettings]);

  return { lyricSettings, setLyricSettings };
}
