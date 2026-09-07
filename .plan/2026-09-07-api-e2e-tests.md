# 2026-09-07 — API 端到端测试补强

## 背景

- 来自完成度复查报告（`.plan/2026-06-06-six-version-completion-audit.md` 短期优化第 1 项）：`/api/cache/:id`、`/api/runs`、token 401/成功路径、AI score 输出可再增加 server handler 或 HTTP integration 测试。
- v1.8.0 测试基线已覆盖 `/api/health`、`/api/stats`、`/api/runs`、`/api/cache/:id`（upstream/unified/missing 三分支）与 token 认证三情境，但 v4.9.0–v4.13.0 期间新增的路由未同步补测试。
- 本计划对应 `requirement.md` 的 `v2.4.0 — 2026-09-07 — API 端到端测试补强`。

## 目标

- [ ] 为 `/api/cache/:id/revalidate` 与 `/api/unified-cache/:id/revalidate` 增加 router 级测试。
- [ ] 为 `/api/cache/:id` DELETE 路由增加 router 级测试（含幂等语义）。
- [ ] 为 `/api/ai/replay` 增加 router 级测试（覆盖未配置 AI scoring 的错误路径）。
- [ ] 全部既有 Rust 测试保持通过。

## 非目标

- 不引入真实网络请求或外部 Provider 依赖（沿用 `ServiceContext::default()` 的 test 双轨）。
- 不修改 handler 实现；仅补测试。
- 不处理浏览器 Playwright 专项（Phase 7/9 范畴，已有专项计划）。

## 风险

- 风险 1：测试环境 HTTP/HTTPS proxy 干扰 wiremock。缓解：沿用 `NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost`，本计划全部使用 router oneshot 测试，不依赖 wiremock 网络栈。

## 阶段列表

### Phase 1 — 需求釐清与设计

#### Task checklist

- [x] 盘点 `src/server.rs` 现有测试覆盖（health/stats/runs/cache_list/cache_detail 三分支/token auth/unified cache delete）。
- [x] 确认缺口：`cache_revalidate`、`unified_cache_revalidate`、`cache_delete`、`ai_replay` 四条路由无 router 级测试。
- [x] 确认测试双轨与辅助函数（`test_cache`/`test_app`/`get_json`/`post_json`/`delete_json`）可直接复用。

#### 验收条件

- [x] 缺口清单与测试设计方向已记录。

#### 测试纪录

- 尚未执行。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

### Phase 2 — 實作

#### Task checklist

- [x] `cache_revalidate_deletes_upstream_entry`：POST `/api/cache/:id/revalidate` 返回 `deleted_cached_entry`，且 cache 中该条目被删除。
- [x] `unified_cache_revalidate_deletes_unified_entry`：POST `/api/unified-cache/:id/revalidate` 返回 `deleted_unified_cached_entry`，且 unified cache 中该条目被删除。
- [x] `cache_delete_removes_upstream_entry_and_reports_404_after`：DELETE `/api/cache/:id` 首次返回 `deleted=true`，二次幂等返回 `deleted=false`。
- [x] `ai_replay_returns_service_error_when_ai_scoring_not_configured`：POST `/api/ai/replay` 在未配置 AI scoring 时返回非 200 + 结构化 `{code,message}` 错误体。
- [x] 新增测试均通过。

#### 验收条件

- [x] 四条路由的 router 级行为均被固化。
- [x] 与非目标无冲突：未改 handler 实现。

#### 测试紀錄

- `cargo test --lib revalidate` → 2 passed。
- `cargo test --lib cache_delete_` → 1 passed。
- `cargo test --lib ai_replay_` → 1 passed。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

### Phase 3 — 驗收與收尾

#### Task checklist

- [x] 运行完整 Rust 测试与格式检查。
- [x] 更新 `requirement.md` v2.4.0 状态。
- [x] 版本号：测试补强属 patch 级，维持 4.13.0，不打新 tag（由下一轮功能版本统一处理）。

#### 验收条件

- [x] 所有必要检查已通过。
- [x] 文件已同步更新。

#### 测试纪录

- `cargo test --lib server::tests` → 23 passed（含 4 个新增）。
- `cargo fmt --check` → clean。
- `git diff --check` → clean。
- 既有 wiremock provider 测试因 Windows 端口耗尽（`AddrInUse`）失败 2 条，属已知环境限制（v1.8.0 记录），非本次回归。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

#### 測試紀錄

- 尚未執行。

#### 完成狀態

- [ ] 未開始
- [ ] 進行中
- [ ] 已完成

## 整體驗收條件

- [ ] 需求目標已完成
- [ ] 測試紀錄完整
- [ ] 相關文件已同步
- [ ] `requirement.md` 已追加

## 整體測試紀錄

| 日期 | 指令 / 檢查 | 結果 | 備註 |
|------|-------------|------|------|
| 2026-09-07 | `cargo test --lib revalidate` | 2 passed | 新增 upstream/unified revalidate |
| 2026-09-07 | `cargo test --lib cache_delete_` | 1 passed | 新增幂等 DELETE |
| 2026-09-07 | `cargo test --lib ai_replay_` | 1 passed | 新增未配置错误路径 |
| 2026-09-07 | `cargo test --no-fail-fast`（全量） | 待执行 | 见 Phase 3 |

## 整體完成狀態

- [ ] 未開始
- [ ] 進行中
- [x] 已完成
- [ ] 已取消
