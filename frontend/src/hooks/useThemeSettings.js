import { useEffect, useMemo, useState } from 'react';

export const themePresets = {
  'cyber-dark': {
    id: 'cyber-dark',
    labelKey: 'themeCyberDark',
    descriptionKey: 'themeCyberDarkHint',
    className: 'theme-cyber-dark',
  },
  midnight: {
    id: 'midnight',
    labelKey: 'themeMidnight',
    descriptionKey: 'themeMidnightHint',
    className: 'theme-midnight',
  },
  'minimal-light': {
    id: 'minimal-light',
    labelKey: 'themeMinimalLight',
    descriptionKey: 'themeMinimalLightHint',
    className: 'theme-minimal-light',
  },
};

export const defaultThemeSettings = {
  preset: 'cyber-dark',
};

const storageKey = 'rosettrism-theme-settings';

export function readThemeSettings() {
  try {
    const stored = JSON.parse(localStorage.getItem(storageKey) || 'null');
    const preset = themePresets[stored?.preset] ? stored.preset : defaultThemeSettings.preset;
    return { ...defaultThemeSettings, ...(stored || {}), preset };
  } catch {
    return defaultThemeSettings;
  }
}

export function useThemeSettings() {
  const [themeSettings, setThemeSettings] = useState(readThemeSettings);
  const themePreset = themePresets[themeSettings.preset] || themePresets[defaultThemeSettings.preset];

  useEffect(() => {
    localStorage.setItem(storageKey, JSON.stringify(themeSettings));
  }, [themeSettings]);

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = themePreset.id;
    root.classList.remove(...Object.values(themePresets).map((preset) => preset.className));
    root.classList.add(themePreset.className);
    return () => {
      root.classList.remove(themePreset.className);
      delete root.dataset.theme;
    };
  }, [themePreset]);

  const themeOptions = useMemo(() => Object.values(themePresets), []);

  return { themeSettings, setThemeSettings, themePreset, themeOptions };
}
