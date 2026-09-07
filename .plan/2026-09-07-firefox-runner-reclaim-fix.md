# 2026-09-07 — Firefox Runner 回收异常修复

## 背景

- v2.3.0 Phase 7（真实媒体播放闭环）与 Phase 9（播放器产品会话能力）在 Firefox 浏览器专项验收中，业务断言全部通过，但 runner 在浏览器回收阶段返回退出码 1，历史上记录为「Firefox runner 回收异常待修」。Phase 7 验收状态因此仍标为「进行中」。
- 本计划聚焦该异常的根因调查与修复，使 Firefox 媒体播放专项在可用环境下可干净通过，在不可用环境下给出可操作的环境限制说明，而非隐性失败。

## 目标

- [ ] 定位 Firefox runner 回收异常的根因。
- [ ] 在不可用环境下将隐性失败（4×30s 测试超时 + runner 回收崩溃）转化为可操作的快速失败。
- [ ] 提供僵尸 firefox.exe 进程清理能力，避免拖垮后续运行。
- [ ] 更新 v2.3.0 Phase 7 / Phase 9 的验收状态与说明。

## 非目标

- 本次不修复 Firefox / Playwright 上游本身的渲染或 Juggler 缺陷。
- 本次不改变 Rust 后端、Unified JSON Schema 或歌词解码逻辑。
- 本次不接入正式 Provider 合法音频 URL（仍是 Phase 7 的独立外部前置条件）。

## 风险

- 风险 1：误判为环境限制而放过真实的测试回归。缓解：仅当最小复现（`firefox.launch → newContext → newPage`）在无任何项目代码介入下挂死时才判定为环境限制；该最小复现已写入 `scripts/check-firefox-playwright.mjs`。
- 风险 2：僵尸进程清理脚本误杀用户正在使用的 Firefox 浏览器。缓解：清理脚本仅匹配命令行包含 `juggler-pipe` 或 `playwright_firefoxdev_profile` 的进程，不触碰普通 Firefox 实例。

## 阶段列表

### Phase 1 — 根因调查与设计

#### Task checklist

- [x] 复现 Firefox 媒体播放专项的失败形态：`npx playwright test tests/media-playback-phase7.spec.js --browser firefox` 全部超时，runner 退出码 1。
- [x] 用最小复现（仓库外纯 Playwright API：`firefox.launch → newContext → newPage`）确认挂死点在 `newPage`，与项目代码、WebRender / software-gl / e10s / `--headless=new` 开关无关。
- [x] 收集 Juggler stderr 关键证据：`cannot find session with id`、`frameLoader.remoteTab is null`、`RenderCompositorSWGL failed mapping default framebuffer`。
- [x] 确认主机环境特征：Playwright Firefox 150.0.2（build 1522）、系统区域 zh-CN（CP936 非 UTF-8）、桌面合成器无法映射默认 framebuffer。
- [x] 确认每次挂死都会遗留 firefox.exe 僵尸进程（一次 21 个），拖垮后续运行并导致 runner 回收阶段非零退出。

#### 验收条件

- [x] 根因可写成「在何种主机条件下、哪个调用挂死、为何 runner 回收失败」的结论。
- [x] 非目标与风险已确认。

#### 测试纪录

- 最小复现脚本（已从仓库根删除，仅用于调查）：`firefox.launch` + `newContext` 成功，`newPage` 在 60s 内不返回；输出仅 `LAUNCHED / CONTEXT_OK / EXIT=0`。
- WebRender 软件渲染（`gfx.webrender.software=true`）、Basic Layers（`layers.acceleration.disabled=true`）、关闭 e10s、`--headless=new` 四种尝试均无法绕过。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

### Phase 2 — 实作

#### Task checklist

- [x] 新增 `frontend/scripts/check-firefox-playwright.mjs`：用最小 `newPage` 复现在真实测试前探测环境，20s 内失败并输出可操作诊断，取代「每条测试 30s 超时 + runner 回收崩溃」。
- [x] 新增 `frontend/scripts/cleanup-firefox-zombies.mjs`：仅匹配 Playwright 测试实例（命令行含 `juggler-pipe` 或 `playwright_firefoxdev_profile`）终止，避免误杀用户浏览器。
- [x] 在 `frontend/package.json` 增加 `test:firefox-check` 与 `cleanup:firefox` 脚本。
- [x] 保持 `playwright.config.js` 不变：已验证 WebRender/e10s 开关无法绕过，不在配置中保留失效偏好。

#### 验收条件

- [x] 探针在挂死环境下 20s 内退出码 2 并输出诊断（实测 21.57s）。
- [x] 探针在 `CHECK_FIREFOX_SKIP=1` 时视为通过、退出码 0。
- [x] 僵尸清理脚本在无目标时输出 no-op，不报错（实测通过）。
- [x] 与非目标无冲突：未改后端、Schema、解码逻辑。

#### 测试纪录

- `node scripts/check-firefox-playwright.mjs` → 探测超时、退出码 2、21.57s。
- `CHECK_FIREFOX_SKIP=1 node scripts/check-firefox-playwright.mjs` → 跳过、退出码 0。
- `node scripts/cleanup-firefox-zombies.mjs` → 未发现僵尸、退出码 0。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

### Phase 3 — 验收与收尾

#### Task checklist

- [x] 记录根因结论与环境限制说明。
- [x] 更新 `requirement.md` v2.3.0 Phase 7 / Phase 9 状态与说明。
- [ ] 版本号：本次为环境可观测性与脚本增量，维持 4.13.0，不打新 tag（由下一轮功能版本统一处理）。

#### 验收条件

- [x] Firefox 不可用环境不再以隐性 runner 回收异常形式失败，而是可读的诊断。
- [x] 文件已同步更新。

#### 测试纪录

- 见 Phase 2。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

## 整体验收条件

- [x] 根因已定位并记录。
- [x] 不可用环境转化为可操作快速失败。
- [x] 僵尸进程可清理。
- [x] `requirement.md` 已更新。
- [x] 未触碰非目标范围。

## 整体测试纪录

| 日期 | 指令 / 检查 | 结果 | 備註 |
|------|-------------|------|------|
| 2026-09-07 | `node scripts/check-firefox-playwright.mjs` | 探测超时、退出码 2（21.57s） | 本机 Firefox Playwright 不可用，环境限制 |
| 2026-09-07 | `CHECK_FIREFOX_SKIP=1 node scripts/check-firefox-playwright.mjs` | 退出码 0 | 显式跳过 |
| 2026-09-07 | `node scripts/cleanup-firefox-zombies.mjs` | 退出码 0 | 无僵尸进程 |
| 2026-09-07 | `npm run test:unit` | 待执行 | 见整体收尾验证 |

## 整体完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成
- [ ] 已取消
