// Firefox Playwright 健康检查探针。
//
// 背景（2026-09-07 复盘）：Windows 上 Playwright 自带 Firefox 在部分主机
// （GPU 驱动 / CP936 非 UTF-8 区域设置）会出现 browserContext.newPage 永久
// 挂死：launch 与 newContext 正常返回，但内容进程从未真正就绪
// （Juggler 报 "cannot find session with id"、页面层报
// "frameLoader.remoteTab is null"、GFX1 报
// "RenderCompositorSWGL failed mapping default framebuffer"）。
//
// 后果：每个测试用 30s 超时失败后，Playwright 无法回收已死上下文，runner
// 在浏览器回收阶段以非零码退出（历史上记录为 "Firefox runner 回收异常"），
// 并遗留大量 firefox.exe 僵尸进程拖垮后续运行。
//
// 本脚本用一个最小复现（launch → newContext → newPage → data: URL 求值）
// 在真实测试前探测该环境问题，并以可操作的诊断信息快速失败，替代
// "每条测试超时 + runner 回收阶段崩溃" 的隐性失败模式。WebRender 软件
// 渲染、Basic Layers、关闭 e10s、--headless=new 均无法绕过（已验证），
// 因此判定为环境限制，而非项目代码缺陷。
//
// 用法：
//   node scripts/check-firefox-playwright.mjs            # 探测，失败退出码 2
//   CHECK_FIREFOX_SKIP=1 node ...                          # 直接跳过（视为通过）
//
// 退出码：0 = Firefox 可用或已显式跳过；2 = 探测失败（环境限制）。

import { firefox } from '../node_modules/playwright/index.mjs';

const PROBE_TIMEOUT_MS = 20_000;

if (process.env.CHECK_FIREFOX_SKIP === '1') {
  console.log('[check-firefox] CHECK_FIREFOX_SKIP=1，跳过 Firefox 探测。');
  process.exit(0);
}

function zombieHint() {
  const isWindows = process.platform === 'win32';
  return [
    '若探测失败，请检查：',
    '1. 是否有遗留的 firefox.exe 僵尸进程拖垮新实例：',
    isWindows
      ? '   powershell -NoProfile -Command "Get-Process firefox -ErrorAction SilentlyContinue | Stop-Process -Force"'
      : '   pkill -f playwright_firefoxdev_profile || true',
    '2. 主机 GPU 驱动 / 系统区域设置（非 UTF-8 代码页如 CP936）会触发此问题，',
    '   已知 WebRender/software-gl/e10s 开关均无法绕过。',
    '3. 明确要以环境限制跳过时使用 CHECK_FIREFOX_SKIP=1，并在记录中注明原因。',
  ].join('\n');
}

const timeout = setTimeout(() => {
  console.error('[check-firefox] 探测超时：browserContext.newPage 挂死超过 '
    + `${PROBE_TIMEOUT_MS}ms，判定为环境限制。`);
  console.error(zombieHint());
  process.exit(2);
}, PROBE_TIMEOUT_MS);
timeout.unref();

try {
  const browser = await firefox.launch({ headless: true, timeout: PROBE_TIMEOUT_MS });
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto('data:text/html,<h1>ok</h1>');
  const text = await page.textContent('h1');
  await browser.close();
  clearTimeout(timeout);
  if (text === 'ok') {
    console.log('[check-firefox] Firefox Playwright 探测通过。');
    process.exit(0);
  }
  console.error(`[check-firefox] 探测返回异常内容：${JSON.stringify(text)}`);
  process.exit(2);
} catch (error) {
  clearTimeout(timeout);
  console.error(`[check-firefox] 探测失败：${error && error.message ? error.message.split('\n')[0] : error}`);
  console.error(zombieHint());
  process.exit(2);
}
