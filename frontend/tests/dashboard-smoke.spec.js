import { expect, test } from '@playwright/test';

async function mockApi(page) {
  await page.route('**/api/health', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ ok: true, version: 'smoke', cache: true }),
  }));
  await page.route('**/api/stats', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({
      cache: {
        upstream_entries: 0,
        unified_entries: 0,
        ai_score_entries: 0,
        fetch_run_entries: 0,
        fresh_upstream_entries: 0,
        expired_upstream_entries: 0,
      },
      ai_scores: [],
      fetch_runs: [],
      fetch_run_status_counts: [],
    }),
  }));
  await page.route('**/api/cache', (route) => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({ entries: [] }),
  }));
}

test('Dashboard loads, Settings token input works, and Fetch renders', async ({ page }) => {
  await mockApi(page);
  await page.goto('/');

  await expect(page.getByRole('heading', { name: '缓存健康' })).toBeVisible();
  await expect(page.getByText('smoke')).toBeVisible();

  await page.getByRole('button', { name: '设置' }).click();
  const tokenInput = page.getByPlaceholder('ROSETTRISM_SERVER_TOKEN');
  await expect(tokenInput).toBeVisible();
  await tokenInput.fill('smoke-token');
  await expect(tokenInput).toHaveValue('smoke-token');

  await page.getByRole('button', { name: '获取' }).click();
  await expect(page.getByRole('heading', { name: '获取' })).toBeVisible();
  await expect(page.getByPlaceholder('关键词 / 歌名 / 艺人')).toBeVisible();
});
