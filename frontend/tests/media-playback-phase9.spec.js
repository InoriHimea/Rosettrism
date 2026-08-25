import { expect, test } from '@playwright/test';

const payload = {
  document: {
    meta: { title: 'Phase 9 播放会话', artist: 'Rosettrism', input_format: 'lrc' },
    lines: [
      { start_ms: 0, duration_ms: 2000, text: '队列与系统媒体控制', words: [] },
      { start_ms: 2000, duration_ms: 2000, text: '恢复音量与播放位置', words: [] },
      { start_ms: 4000, duration_ms: 2000, text: '结束后按模式切换', words: [] },
    ],
  },
};

const audioSource = '/tests/fixtures/media-clock-tone.wav';

async function installHarness(page, { persistSession = false, includeBroken = false } = {}) {
  await page.addInitScript(({ injectedPayload, source, shouldPersist, broken }) => {
    const handlers = {};
    Object.defineProperty(navigator, 'mediaSession', {
      configurable: true,
      value: {
        metadata: null,
        setActionHandler(action, handler) {
          handlers[action] = handler;
        },
      },
    });
    window.MediaMetadata = class MediaMetadata {
      constructor(metadata) {
        Object.assign(this, metadata);
      }
    };
    window.__MEDIA_SESSION_TEST__ = {
      handlers,
      invoke(action, details = {}) {
        return handlers[action]?.(details);
      },
    };
    const queue = [
      { id: 'track-a', src: source, title: '音轨 A', artist: 'Rosettrism', durable: true },
      { id: 'track-b', src: `${source}?track=b`, title: '音轨 B', artist: 'Rosettrism', durable: true },
    ];
    if (broken) {
      queue.unshift({ id: 'broken', src: '/tests/fixtures/missing-audio.wav', title: '损坏音轨', durable: false });
    }
    window.__LYRIC_MEDIA_HARNESS__ = {
      payload: injectedPayload,
      queue,
      persistSession: shouldPersist,
      settings: { ambientEffects: false, lowDistraction: true, stage3D: false },
    };
  }, { injectedPayload: payload, source: audioSource, shouldPersist: persistSession, broken: includeBroken });
}

async function openHarness(page) {
  const diagnostics = [];
  page.on('pageerror', (error) => diagnostics.push(`pageerror: ${error.stack || error.message}`));
  page.on('requestfailed', (request) => diagnostics.push(`requestfailed: ${request.url()} ${request.failure()?.errorText || ''}`));
  page.on('response', (response) => {
    if (response.status() >= 400) {
      diagnostics.push(`response: ${response.status()} ${response.url()}`);
    }
  });
  await page.goto('/media-playback-harness.html');
  try {
    await expect(page.getByTestId('player-session-bar')).toBeVisible({ timeout: 15_000 });
  } catch (error) {
    const root = await page.locator('#root').innerHTML().catch(() => '<missing root>');
    throw new Error(`${error.message}\nRoot: ${root}\nDiagnostics:\n${diagnostics.join('\n')}`);
  }
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__?.durationMs() || 0)).toBeGreaterThan(5000);
}

test('queue controls switch real audio without stale lyric time', async ({ page }) => {
  await installHarness(page);
  await openHarness(page);

  await page.evaluate(() => {
    window.__LYRIC_MEDIA_HARNESS_API__.seek(4200);
    window.__LYRIC_MEDIA_HARNESS_API__.next();
  });
  await expect(page.getByTestId('media-source-index')).toHaveText('1');
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.currentMs())).toBeLessThan(250);
  await expect(page.getByTestId('player-session-bar')).toContainText('音轨 B');

  await page.getByRole('button', { name: '上一首' }).click();
  await expect(page.getByTestId('media-source-index')).toHaveText('0');
});

test('repeat-one and repeat-all end handling follows session mode', async ({ page }) => {
  await installHarness(page);
  await openHarness(page);

  await page.evaluate(() => {
    window.__LYRIC_MEDIA_HARNESS_API__.setMode('repeat-one');
    document.querySelector('audio').dispatchEvent(new Event('ended'));
  });
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.sessionState().currentId)).toBe('track-a');
  await expect(page.getByTestId('player-session-bar')).toHaveAttribute('data-playback-mode', 'repeat-one');

  await page.evaluate(() => {
    window.__LYRIC_MEDIA_HARNESS_API__.setMode('repeat-all');
    window.__LYRIC_MEDIA_HARNESS_API__.selectTrack('track-b', false);
    document.querySelector('audio').dispatchEvent(new Event('ended'));
  });
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.sessionState().currentId)).toBe('track-a');
});

test('volume mute rate and durable position restore after reload', async ({ page }) => {
  await installHarness(page, { persistSession: true });
  await openHarness(page);

  await page.evaluate(() => {
    const api = window.__LYRIC_MEDIA_HARNESS_API__;
    api.selectTrack('track-b', false);
    api.setVolume(0.35);
    api.toggleMuted();
    api.setPlaybackRate(1.5);
    api.seek(2300);
  });
  await expect.poll(() => page.evaluate(() => {
    const state = window.__LYRIC_MEDIA_HARNESS_API__.sessionState();
    return [state.currentId, state.volume, state.muted, state.playbackRate, Math.round(state.positionMs / 100) * 100];
  })).toEqual(['track-b', 0.35, true, 1.5, 2300]);
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem('rosettrism-player-session')).positionMs)).toBeGreaterThan(2000);

  await page.reload();
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__?.sessionState().currentId)).toBe('track-b');
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.sessionState().positionMs)).toBeGreaterThan(2000);
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.mediaState().volume)).toBe(0.35);
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.mediaState().muted)).toBe(true);
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.mediaState().playbackRate)).toBe(1.5);
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.currentMs())).toBeGreaterThan(2000);
  await expect(page.locator('.lyric-playback-card')).toHaveAttribute('data-playback-time-ms', /2\d\d\d/);
});

test('Media Session actions map to the same queue and media clock truth', async ({ page }) => {
  await installHarness(page);
  await openHarness(page);

  await expect.poll(() => page.evaluate(() => Object.keys(window.__MEDIA_SESSION_TEST__.handlers).sort())).toEqual([
    'nexttrack', 'pause', 'play', 'previoustrack', 'seekbackward', 'seekforward', 'seekto',
  ]);
  await expect.poll(() => page.evaluate(() => navigator.mediaSession.metadata?.title)).toBe('音轨 A');

  await page.evaluate(() => window.__MEDIA_SESSION_TEST__.invoke('seekto', { seekTime: 2.4 }));
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.currentMs())).toBeGreaterThan(2200);
  await page.evaluate(() => window.__MEDIA_SESSION_TEST__.invoke('nexttrack'));
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.sessionState().currentId)).toBe('track-b');
  await expect.poll(() => page.evaluate(() => navigator.mediaSession.metadata?.title)).toBe('音轨 B');
});

test('audio errors expose retry separately and mobile controls stay usable', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await installHarness(page, { includeBroken: true });
  await page.goto('/media-playback-harness.html');

  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__?.sessionState().status)).toBe('error');
  await expect(page.getByRole('button', { name: '重试音频' })).toBeVisible();
  await page.getByRole('button', { name: '重试音频' }).click();
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.sessionState().retryCount)).toBeGreaterThan(0);

  const metrics = await page.evaluate(() => {
    const bar = document.querySelector('[data-testid="player-session-bar"]');
    const buttons = [...bar.querySelectorAll('button')];
    return {
      overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      buttonSizes: buttons.map((button) => ({ width: button.getBoundingClientRect().width, height: button.getBoundingClientRect().height })),
    };
  });
  expect(metrics.overflow).toBeLessThanOrEqual(1);
  expect(metrics.buttonSizes.every(({ width, height }) => width >= 44 && height >= 44)).toBe(true);
});
