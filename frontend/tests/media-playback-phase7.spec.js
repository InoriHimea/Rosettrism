import { expect, test } from '@playwright/test';

const payload = {
  document: {
    meta: { title: 'Phase 7 媒体时钟', artist: 'Rosettrism', input_format: 'lrc' },
    lines: [
      { start_ms: 0, duration_ms: 2000, text: '真实音频驱动第一句', words: [] },
      { start_ms: 2000, duration_ms: 2000, text: '暂停拖动倍速保持同步', words: [] },
      { start_ms: 4000, duration_ms: 2000, text: '媒体结束进入结束状态', words: [] },
    ],
  },
};

const audioSource = '/tests/fixtures/media-clock-tone.wav';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ injectedPayload, source }) => {
    window.__LYRIC_MEDIA_HARNESS__ = {
      payload: injectedPayload,
      sources: [source, `${source}?alternate=1`],
      settings: { ambientEffects: false, lowDistraction: true, stage3D: false },
    };
  }, { injectedPayload: payload, source: audioSource });
  await page.goto('/media-playback-harness.html');
  await expect(page.locator('.lyric-playback-card')).toBeVisible();
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__?.durationMs() || 0)).toBeGreaterThan(5000);
});

test('real audio drives play pause seek and lyric state from media.currentTime', async ({ page }) => {
  await page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.play());
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.currentMs())).toBeGreaterThan(100);
  await expect(page.locator('.lyric-playback-card')).toHaveAttribute('data-playback-phase', 'singing');

  await page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.pause());
  const pausedAt = await page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.currentMs());
  await page.waitForTimeout(250);
  const stillPausedAt = await page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.currentMs());
  expect(Math.abs(stillPausedAt - pausedAt)).toBeLessThan(40);

  await page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.seek(2300));
  await expect(page.locator('.lyric-playback-card')).toHaveAttribute('data-playback-time-ms', /2[23]\d\d/);
  await expectBaseText(page.locator('.lyric-line-active'), '暂停拖动倍速保持同步');
});

test('playback rate changes media truth and remains visible to lyric controls', async ({ page }) => {
  await page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.setPlaybackRate(2));
  await expect(page.locator('.lyric-playback-card')).toHaveAttribute('data-media-rate', '2');
  await expect(page.locator('.lyric-rate-toggle')).toHaveText('2x');

  await page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.play());
  const start = await page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.currentMs());
  await page.waitForTimeout(350);
  const end = await page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.currentMs());
  expect(end - start).toBeGreaterThan(450);
});

test('switching audio source resets media time without stale lyric state', async ({ page }) => {
  await page.evaluate(() => {
    window.__LYRIC_MEDIA_HARNESS_API__.seek(4300);
    window.__LYRIC_MEDIA_HARNESS_API__.switchSource(1);
  });
  await expect(page.getByTestId('media-source-index')).toHaveText('1');
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.currentMs())).toBeLessThan(200);
  await expect(page.locator('.lyric-playback-card')).not.toHaveAttribute('data-playback-phase', 'ended');
});

test('media end and replay reset the lyric state deterministically', async ({ page }) => {
  const durationMs = await page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.durationMs());
  await page.evaluate((value) => window.__LYRIC_MEDIA_HARNESS_API__.seek(value), durationMs);
  await expect(page.locator('.lyric-playback-card')).toHaveAttribute('data-playback-phase', 'ended');

  await page.getByRole('button', { name: '重播' }).click();
  await expect.poll(() => page.evaluate(() => window.__LYRIC_MEDIA_HARNESS_API__.currentMs())).toBeLessThan(1000);
  await expect(page.locator('.lyric-playback-card')).not.toHaveAttribute('data-playback-phase', 'ended');
});

async function expectBaseText(line, expected) {
  const actual = await line.locator('.lyric-progress-base').evaluateAll((nodes) => (
    nodes.map((node) => node.textContent || '').join('')
  ));
  expect(actual).toBe(expected);
}
