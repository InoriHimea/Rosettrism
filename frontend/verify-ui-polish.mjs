#!/usr/bin/env node
import { chromium } from 'playwright';

const baseUrl = 'http://localhost:5173';
const views = ['overview', 'fetch', 'cache', 'inspector', 'settings'];
const themes = ['default', 'theme-midnight', 'theme-minimal-light'];

(async () => {
  const browser = await chromium.launch({ headless: false });
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const page = await context.newPage();

  console.log('🎨 UI/UX 精修验证开始\n');

  // 等待服务器就绪
  await page.goto(baseUrl, { waitUntil: 'domcontentloaded', timeout: 15000 });
  await page.waitForTimeout(1000);

  for (const theme of themes) {
    console.log(`\n📐 主题: ${theme}`);

    // 切换主题
    if (theme !== 'default') {
      await page.evaluate((t) => {
        document.documentElement.className = t;
      }, theme);
      await page.waitForTimeout(300);
    }

    for (const view of views) {
      console.log(`  ✓ 验证 ${view} view`);

      // 导航到对应 view
      await page.goto(`${baseUrl}/#${view}`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(500);

      // 检查触控目标尺寸（按钮至少 44×44px）
      const buttons = await page.locator('button, .button, .button-icon').all();
      for (const btn of buttons) {
        const box = await btn.boundingBox();
        if (box && (box.width < 44 || box.height < 44)) {
          const text = await btn.textContent();
          console.warn(`    ⚠️  触控目标过小: ${text?.trim() || 'unnamed'} (${box.width}×${box.height}px)`);
        }
      }

      // 检查文本对比度（通过 Accessibility API）
      const snapshot = await page.accessibility.snapshot();
      const checkContrast = (node) => {
        if (node.role === 'text' && node.valueText) {
          // Playwright accessibility API 不直接提供对比度值，这里仅记录文本节点存在
          // 实际对比度需浏览器 DevTools 手动核查
        }
        if (node.children) {
          node.children.forEach(checkContrast);
        }
      };
      if (snapshot) checkContrast(snapshot);
    }
  }

  // 移动端触控验证（375px 宽度）
  console.log(`\n📱 移动端 375px 触控验证`);
  await page.setViewportSize({ width: 375, height: 667 });
  for (const view of views) {
    await page.goto(`${baseUrl}/#${view}`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(300);

    const buttons = await page.locator('button, .button, .button-icon').all();
    let tooSmallCount = 0;
    for (const btn of buttons) {
      const box = await btn.boundingBox();
      if (box && (box.width < 44 || box.height < 44)) tooSmallCount++;
    }
    console.log(`  ${view}: ${tooSmallCount > 0 ? '⚠️ ' + tooSmallCount + ' 个' : '✓'} 按钮尺寸不足`);
  }

  console.log('\n✨ 验证完成。浏览器保持打开，请手动检查：');
  console.log('  1. 各 view 间距对齐 8px 网格');
  console.log('  2. 三套主题颜色无样式断层');
  console.log('  3. DevTools Accessibility 面板检查对比度 ≥4.5:1');
  console.log('  4. 测试 prefers-reduced-motion (DevTools Rendering > Emulate CSS prefers-reduced-motion)');
  console.log('\n按 Ctrl+C 退出。');

  await page.waitForTimeout(300000); // 保持 5 分钟供手动检查
  await browser.close();
})();
