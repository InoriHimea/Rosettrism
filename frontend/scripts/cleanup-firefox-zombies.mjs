// 清理 Playwright Firefox 测试遗留的僵尸 firefox.exe 进程。
//
// 当 browserContext.newPage 挂死导致测试超时时，Playwright 无法回收浏览器
// 进程，会留下成批 firefox.exe 子进程。这些僵尸进程会拖垮后续运行（历史
// 上表现为 "Firefox runner 回收异常"，runner 在 teardown 阶段退出码 1）。
//
// 本脚本只终止 Playwright 测试实例（命令行包含 -juggler-pipe 或
// playwright_firefoxdev_profile），不会误杀用户自己的 Firefox 浏览器。
//
// 用法：node scripts/cleanup-firefox-zombies.mjs

import { execFileSync } from 'node:child_process';

if (process.platform !== 'win32') {
  // 非 Windows 平台目前没有观察到该问题；保持 no-op 以便跨平台脚本可复用。
  console.log('[cleanup-firefox] 非 Windows 平台，跳过僵尸进程清理。');
  process.exit(0);
}

const output = execFileSync(
  'powershell',
  [
    '-NoProfile',
    '-Command',
    "Get-CimInstance Win32_Process -Filter \"Name='firefox.exe'\" | Where-Object { $_.CommandLine -match 'juggler-pipe|playwright_firefoxdev_profile' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force; $_.ProcessId }",
  ],
  { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] },
);

const killed = output.trim().split(/\r?\n/).filter(Boolean);
if (killed.length > 0) {
  console.log(`[cleanup-firefox] 已终止 ${killed.length} 个 Playwright Firefox 僵尸进程：${killed.join(', ')}`);
} else {
  console.log('[cleanup-firefox] 未发现 Playwright Firefox 僵尸进程。');
}
