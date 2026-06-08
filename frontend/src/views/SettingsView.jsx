import { defaultLyricSettings, resolveLyricGradient } from '../lyricPlayback.js';

export function SettingsView({ t, language, setLanguage, lyricSettings, setLyricSettings, aiSettings, setAiSettings, serverToken, setServerToken, themeSettings, setThemeSettings, themeOptions, payload }) {
  const previewStyle = {
    '--lyric-solid-color': lyricSettings.solidColor,
    '--lyric-gradient': resolveLyricGradient(lyricSettings.colorPreset),
    '--lyric-stage-background': lyricSettings.stageBackgroundColor || defaultLyricSettings.stageBackgroundColor,
  };

  return (
    <section className="panel split-panel">
      <div className="settings-stack">
        <h2>{t.settings}</h2>
        <label className="field-label">
          {t.language}
          <select value={language} onChange={(event) => setLanguage(event.target.value)}>
            <option value="zh">{t.chinese}</option>
            <option value="en">{t.english}</option>
          </select>
        </label>

        <div className="settings-group theme-settings-group">
          <strong>{t.theme}</strong>
          <p className="hint">{t.themeHint}</p>
          <div className="theme-preset-grid" role="radiogroup" aria-label={t.themePreset}>
            {themeOptions.map((preset) => (
              <label className={`theme-preset-card ${themeSettings.preset === preset.id ? 'theme-preset-card-active' : ''}`} key={preset.id}>
                <input
                  type="radio"
                  name="theme-preset"
                  value={preset.id}
                  checked={themeSettings.preset === preset.id}
                  onChange={(event) => setThemeSettings({ ...themeSettings, preset: event.target.value })}
                />
                <span className={`theme-swatch theme-swatch-${preset.id}`} aria-hidden="true" />
                <strong>{t[preset.labelKey]}</strong>
                <small>{t[preset.descriptionKey]}</small>
              </label>
            ))}
          </div>
        </div>
        <div className="settings-group">
          <strong>{t.serverAccess}</strong>
          <p className="hint">{t.serverTokenHint}</p>
          <label className="field-label">
            {t.serverToken}
            <input
              type="password"
              value={serverToken}
              autoComplete="off"
              placeholder="ROSETTRISM_SERVER_TOKEN"
              onChange={(event) => setServerToken(event.target.value)}
            />
          </label>
          <button className="button-secondary settings-token-clear" type="button" onClick={() => setServerToken('')} disabled={!serverToken}>
            {t.clearServerToken}
          </button>
        </div>
        <div className="settings-group">
          <strong>{t.aiScoring}</strong>
          <p className="hint">{t.aiScoringHint}</p>
          <label className="field-label settings-checkbox-row">
            <input
              type="checkbox"
              checked={Boolean(aiSettings.enabled)}
              onChange={(event) => setAiSettings({ ...aiSettings, enabled: event.target.checked })}
            />
            {t.aiEnabled}
          </label>
          <label className="field-label">
            {t.aiBaseUrl}
            <input
              type="url"
              value={aiSettings.baseUrl}
              placeholder="https://api.openai.com/v1"
              onChange={(event) => setAiSettings({ ...aiSettings, baseUrl: event.target.value })}
            />
          </label>
          <label className="field-label">
            {t.aiModel}
            <input
              type="text"
              value={aiSettings.model}
              placeholder="gpt-4o-mini"
              onChange={(event) => setAiSettings({ ...aiSettings, model: event.target.value })}
            />
          </label>
          <label className="field-label">
            {t.aiApiKey}
            <input
              type="password"
              value={aiSettings.apiKey}
              autoComplete="off"
              placeholder="sk-..."
              onChange={(event) => setAiSettings({ ...aiSettings, apiKey: event.target.value })}
            />
            <span>{t.aiApiKeyHint}</span>
          </label>
        </div>
        <div className="settings-group">
          <strong>{t.lyricColor}</strong>
          <label className="field-label">
            {t.lyricRenderMode}
            <select
              value={lyricSettings.renderMode || defaultLyricSettings.renderMode}
              onChange={(event) => setLyricSettings({ ...lyricSettings, renderMode: event.target.value })}
            >
              <option value="vertical">{t.lyricRenderVertical}</option>
              <option value="karaoke">{t.lyricRenderKaraoke}</option>
            </select>
          </label>

          <label className="field-label">
            {t.karaokeMotionPreset}
            <select
              value={lyricSettings.motionPreset || defaultLyricSettings.motionPreset}
              onChange={(event) => setLyricSettings({ ...lyricSettings, motionPreset: event.target.value })}
            >
              <option value="cinematic">{t.motionCinematic}</option>
              <option value="snappy">{t.motionSnappy}</option>
              <option value="calm">{t.motionCalm}</option>
            </select>
          </label>
          <label className="field-label settings-checkbox-row">
            <input
              type="checkbox"
              checked={lyricSettings.ambientEffects !== false}
              onChange={(event) => setLyricSettings({ ...lyricSettings, ambientEffects: event.target.checked })}
            />
            {t.lyricAmbientEffects}
          </label>
          <label className="field-label settings-checkbox-row">
            <input
              type="checkbox"
              checked={Boolean(lyricSettings.lowDistraction)}
              onChange={(event) => setLyricSettings({ ...lyricSettings, lowDistraction: event.target.checked })}
            />
            {t.lyricLowDistraction}
          </label>
          <label className="field-label settings-color-row">
            {t.lyricStageBackground}
            <input
              type="color"
              value={lyricSettings.stageBackgroundColor || defaultLyricSettings.stageBackgroundColor}
              onChange={(event) => setLyricSettings({ ...lyricSettings, stageBackgroundColor: event.target.value })}
            />
          </label>
          <label className="field-label">
            {t.lyricColorMode}
            <select
              value={lyricSettings.colorMode}
              onChange={(event) => setLyricSettings({ ...lyricSettings, colorMode: event.target.value })}
            >
              <option value="gradient">{t.gradient}</option>
              <option value="solid">{t.solid}</option>
            </select>
          </label>
          <label className="field-label">
            {t.lyricColorPreset}
            <select
              value={lyricSettings.colorPreset}
              disabled={lyricSettings.colorMode !== 'gradient'}
              onChange={(event) => setLyricSettings({ ...lyricSettings, colorPreset: event.target.value })}
            >
              <option value="qq-prism">{t.qqPrism}</option>
              <option value="aurora">{t.aurora}</option>
              <option value="sunset">{t.sunset}</option>
              <option value="classic">{t.classic}</option>
            </select>
          </label>
          <label className="field-label settings-color-row">
            {t.solidColor}
            <input
              type="color"
              value={lyricSettings.solidColor || defaultLyricSettings.solidColor}
              disabled={lyricSettings.colorMode !== 'solid'}
              onChange={(event) => setLyricSettings({ ...lyricSettings, solidColor: event.target.value })}
            />
          </label>
          <div className={`lyric-color-preview lyric-color-${lyricSettings.colorMode}`} style={previewStyle}>
            Rosettrism Lyrics
          </div>
        </div>
        <p className="hint">{t.cachePath}</p>
      </div>
      <pre className="result compact">{JSON.stringify(payload, null, 2)}</pre>
    </section>
  );
}

