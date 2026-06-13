import { expect, test } from '@playwright/test';

const screenshotOptions = { fullPage: true };

async function mockApi(page) {
  const cacheEntry = {
    id: 101,
    cache_type: 'upstream',
    source: 'qq',
    operation: 'fetch',
    status_code: 200,
    fresh: true,
    body_len: 4096,
    body_hash: 'abcdef1234567890',
    cache_key: 'qq:fetch:visual-smoke',
    created_at: '2026-06-07T00:00:00Z',
    expires_at: '2026-06-14T00:00:00Z',
    metadata: { title: 'Visual Smoke', artist: 'Rosettrism' },
  };

  await page.route('**/api/health', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ ok: true, version: 'smoke', cache: true }),
  }));
  await page.route('**/api/stats', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({
      cache: {
        upstream_entries: 6,
        unified_entries: 1,
        ai_score_entries: 3,
        fetch_run_entries: 7,
        fresh_upstream_entries: 4,
        expired_upstream_entries: 2,
      },
      provider_health: [
        { source: 'qq', status: 'healthy', success_rate: 0.96, warning_rate: 0.04, error_rate: 0, average_duration_ms: 420, sample_size: 25 },
        { source: 'netease', status: 'degraded', success_rate: 0.72, warning_rate: 0.18, error_rate: 0.1, average_duration_ms: 810, sample_size: 18, last_error: 'rate limited' },
      ],
      ai_scores: [
        { score_json: { best_index: 0, scores: [{ index: 0, source: 'qq', ai_score: 8.4 }], model: 'smoke-ai' } },
        { score_json: { best_index: 1, scores: [{ index: 1, source: 'netease', ai_score: 7.2 }], model: 'smoke-ai' } },
        { score_json: { best_index: 0, scores: [{ index: 0, source: 'kugou', ai_score: 9.1 }], model: 'smoke-ai' } },
      ],
      fetch_runs: [
        { id: 1, query: 'Visual Smoke', source: 'qq', mode: 'json', status: 'success', message: 'ok', created_at: '2026-06-07T00:00:00Z' },
      ],
      fetch_run_status_counts: [
        { status: 'success', count: 5 },
        { status: 'warning', count: 1 },
        { status: 'error', count: 1 },
      ],
    }),
  }));
  await page.route('**/api/cache', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ entries: [cacheEntry], unified_entries: [] }),
  }));
  await page.route('**/api/cache/101', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ entry: { ...cacheEntry, body_text_preview: '[00:00] Visual smoke lyric' } }),
  }));
  await page.route('**/api/search', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({
      results: [{ source: 'qq', id: 'smoke-song', title: 'Visual Smoke', artist: 'Rosettrism', duration_ms: 120000, extra: { singing_annotations: [] } }],
      warnings: [],
    }),
  }));
  await page.route('**/api/fetch-result', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({
      document: {
        meta: { title: 'Visual Smoke', artist: 'Rosettrism', source: 'qq', input_format: 'json' },
        lines: [
          { start_ms: 0, duration_ms: 2200, text: 'Visual Smoke - Rosettrism' },
          { start_ms: 2400, duration_ms: 2200, text: 'Neon lyrics come alive' },
          { start_ms: 5200, duration_ms: 2600, text: 'Particles drift behind the stage', translation: '背景粒子缓慢漂浮' },
          { start_ms: 8400, duration_ms: 2400, text: 'Accessibility keeps motion calm' },
        ],
      },
      selectedEntry: { source: 'qq', id: 'smoke-song', title: 'Visual Smoke', artist: 'Rosettrism' },
    }),
  }));
}

test('Dashboard loads, Settings token input works, and Fetch renders', async ({ page }) => {
  await mockApi(page);
  await page.goto('/');

  await expect(page.getByRole('heading', { name: '缓存新鲜度环' })).toBeVisible();
  await expect(page.locator('.metric').filter({ hasText: '版本' }).getByText('smoke', { exact: true })).toBeVisible();

  await page.getByRole('button', { name: '设置' }).click();
  const tokenInput = page.getByPlaceholder('ROSETTRISM_SERVER_TOKEN');
  await expect(tokenInput).toBeVisible();
  await tokenInput.fill('smoke-token');
  await expect(tokenInput).toHaveValue('smoke-token');
  await expect(page.getByRole('radio', { name: /和纸浅色/ })).toBeChecked();

  await page.getByRole('button', { name: '获取' }).click();
  await expect(page.getByRole('heading', { name: '获取' })).toBeVisible();
  await expect(page.getByPlaceholder('歌曲名、歌手、URL 或平台 ID')).toBeVisible();
});

test('visual smoke captures dashboard, settings, karaoke stage, and cache detail', async ({ page }) => {
  await mockApi(page);
  await page.goto('/');

  await expect(page.getByTestId('provider-health-cards')).toBeVisible();
  await expect(page.getByTestId('ai-score-sparkline')).toBeVisible();
  await expect(page.getByTestId('cache-freshness-ring')).toBeVisible();
  await expect(page.getByTestId('fetch-status-distribution')).toBeVisible();
  await page.screenshot({ path: 'playwright-artifacts/visual-dashboard.png', ...screenshotOptions });

  await page.getByRole('button', { name: '设置' }).click();
  await expect(page.getByRole('heading', { name: '设置' })).toBeVisible();
  await expect(page.getByText('主题')).toBeVisible();
  await page.getByText('茶庭暗色', { exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'midnight');
  await page.screenshot({ path: 'playwright-artifacts/visual-settings.png', ...screenshotOptions });

  await page.getByRole('button', { name: '获取' }).click();
  await page.getByPlaceholder('歌曲名、歌手、URL 或平台 ID').fill('Visual Smoke');
  await page.getByRole('button', { name: '搜索' }).click();
  await page.getByRole('button', { name: 'Visual Smoke' }).click();
  await page.getByRole('button', { name: '获取 JSON' }).click();
  const stage = page.getByTestId('karaoke-stage');
  await expect(stage).toBeVisible();
  await expect(stage.locator('.lyric-current-strip')).toHaveText('Visual Smoke - Rosettrism');
  await page.screenshot({ path: 'playwright-artifacts/visual-karaoke-stage.png', ...screenshotOptions });
  await page.getByRole('button', { name: '关闭' }).click();

  await page.getByRole('button', { name: '缓存' }).click();
  await page.locator('.cache-row').filter({ hasText: 'qq #101' }).click();
  await expect(page.getByRole('heading', { name: '缓存' })).toBeVisible();
  const cacheDetail = page.locator('.cache-detail');
  await expect(cacheDetail.getByText('Content preview').or(cacheDetail.getByText('内容预览'))).toBeVisible();
  await expect(cacheDetail.getByText('[00:00] Visual smoke lyric')).toBeVisible();
  await page.screenshot({ path: 'playwright-artifacts/visual-cache-detail.png', ...screenshotOptions });
});
