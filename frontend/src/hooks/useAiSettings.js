import { useEffect, useMemo, useState } from 'react';
import { buildAiScoringPayload } from '../utils/lyricResults.js';

export const defaultAiSettings = {
  enabled: false,
  baseUrl: '',
  apiKey: '',
  model: '',
};

function readAiSettings() {
  try {
    const stored = JSON.parse(localStorage.getItem('rosettrism-ai-settings') || 'null');
    return { ...defaultAiSettings, ...(stored || {}) };
  } catch {
    return defaultAiSettings;
  }
}

export function useAiSettings() {
  const [aiSettings, setAiSettings] = useState(readAiSettings);

  useEffect(() => {
    localStorage.setItem('rosettrism-ai-settings', JSON.stringify(aiSettings));
  }, [aiSettings]);

  const aiScoringPayload = useMemo(() => buildAiScoringPayload(aiSettings), [aiSettings]);

  return { aiSettings, setAiSettings, aiScoringPayload };
}
