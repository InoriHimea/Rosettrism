# 2026-10-10 — SQLite 异步化与工程质量收口

## 背景

- 代码审查发现 5 项工程缺陷：① SQLite 阻塞调用跑在 tokio 异步运行时上（`UpstreamCache` 全部方法同步内联调用，慢查询占用 executor 线程，server 并发时拖累所有请求）；② `[profile.release] panic = "abort"` 与长驻 server 冲突（任何 handler panic 直接杀掉整个进程，tokio 无法隔离 task panic）；③ `service.rs` 单文件约 2475 行，职责混杂；④ `model.rs`/`error.rs` 无单测、前端 i18n 字典无隔离测试；⑤ 根目录缺少 `TODO.md`/`feature.md`，任务与搁置特性无处沉淀。
- SQLite 侧还缺 WAL 与 busy_timeout：dashboard server + CLI cache 管理跨进程并发访问同一 DB 文件时易触发 `SQLITE_BUSY`。
- 本计划对应 `requirement.md` 的 `v5.0.0 — 2026-10-10 — SQLite 异步化与工程质量收口`。

## 目标

- [x] `UpstreamCache` 全部 DB 相关方法 async 化：公共 `async fn` + 私有 `*_sync` 孪生 + `tokio::task::spawn_blocking` offload；纯函数（`cache_key`/`unified_key`）保持同步。
- [x] 连接配置启用 `journal_mode=WAL` + `busy_timeout=5000ms`（保留 `foreign_keys=ON`）。
- [x] `Error` 新增 `Task(#[from] tokio::task::JoinError)` 变体。
- [x] `provider/offline_db.rs` 的同步 DB 查询同样 spawn_blocking 包装。
- [x] 移除 `panic = "abort"`，恢复 unwind（server 模式下单请求 panic 不再杀进程；接受二进制体积小幅增加）。
- [x] `service.rs` 拆分为 `src/service/` 目录：`mod.rs`（类型 + ServiceContext + aggregate 主流程）/ `ai_scoring.rs` / `unified_build.rs` / `fetch_runs.rs` / `tests.rs`；公共 API 不变。
- [x] `src/model.rs` 新增单测（sort_and_fill_durations、serde 往返、snake_case 枚举名、空字段省略、schema_version 默认注入与 back-compat）。
- [x] `src/error.rs` 新增单测（display 文案、io/serde_json `#[from]` 转换、JoinError → Task）。
- [x] 前端 i18n 字典测试 `frontend/src/i18n/__tests__/dictionaries.test.js`（zh/en 键奇偶、非空值、placeholder 一致）；`test:unit` glob 扩展。
- [x] 根目录新建 `TODO.md`、`feature.md`；AGENTS.md 过时描述修正。

## 非目标

- 不拆分 `server.rs`（2312 行）与 `cache.rs`（1721 行）——cache.rs 本轮已叠加 async 改造，避免单轮 churn 过大；server.rs 拆分另行立项。
- 不替换 `rusqlite` 为异步驱动（如 `sqlx`）：spawn_blocking + std Mutex 足够（锁守卫从不跨 await，无死锁风险），避免引入新依赖与迁移成本。
- 不触碰 `provider/mod.rs` 的 `provider_for_with_options` match 主体（保留至插件化 M2）。
- 不做前端 hooks/views 组件测试（需组件测试基础设施，记入 `feature.md`）。
- 不清理 stale 远程分支（约 130 个 `lost_world/worktree-agent-*`，需用户确认）。

## 风险

- 公共 API sync→async 是库破坏性变更：缓解——按仓库规则 bump major（4.13.0 → 5.0.0），全部调用点（cli/server/service/cached_provider/tests）同步更新。
- spawn_blocking 线程池默认上限（512）：缓解——单次 DB 操作毫秒级，dashboard 并发量远低于阈值。
- WAL 模式在旧文件系统（网络盘）可能降级：缓解——WAL 失败时 SQLite 回退 journal 模式不报错，busy_timeout 仍然生效。
- panic=unwind 恢复后二进制体积增加：缓解——release profile 其余瘦身项（opt-level=z/lto/strip）保留，体积增幅极小。
- service.rs 拆分引入可见性/切割事故：缓解——编译器驱动迭代修复 + 全量测试对比（拆分前后均 160 lib tests 全绿）。

## 阶段列表

### Phase 1 — 现状探索与方案确认

#### Task checklist

- [x] 梳理 `UpstreamCache` 全部公共方法与调用点（cached_provider/service/server/cli/offline_db）
- [x] 确认锁守卫从不跨 await（无死锁风险，仅阻塞 executor）
- [x] panic 策略决策：unwind（用户确认）
- [x] service.rs 拆分深度决策：仅拆 service.rs（用户确认）
- [x] 版本策略：API 破坏性变更 → major（5.0.0）

#### 验收条件

- [x] 方案与风险已记录并经用户批准（plan mode）

#### 测试纪录

- 现状基线：拆分前 `cargo test` 160 lib + 14 cli + 1 decode_fixture + 5 unified_schema 全绿。

#### 完成状态

- [x] 已完成

### Phase 2 — 实作

#### Task checklist

- [x] cache.rs：公共 async 方法 + 私有 `*_sync` + 通用 `spawn_blocking` 助手；open 处启用 WAL + busy_timeout
- [x] error.rs：`Task(JoinError)` 变体
- [x] Cargo.toml：移除 `panic = "abort"`；version → 5.0.0
- [x] 调用点全量 `.await` 更新（cached_provider/service/server/cli + 各测试模块）
- [x] offline_db.rs spawn_blocking 包装
- [x] service.rs → `src/service/` 目录拆分（5 个文件）
- [x] model.rs / error.rs 单测新增
- [x] 前端 i18n 测试 + `test:unit` glob
- [x] AGENTS.md 架构描述修正

#### 验收条件

- [x] `cargo check --tests` 0 error、0 真实 warning（仅环境性 hard-link warning）
- [x] `NO_PROXY=... cargo test` 全绿：160+11 lib / 14 cli / 1 decode_fixture / 5 unified_schema
- [x] `npm run test:unit` 42 tests 全过（新增 i18n 3 项）
- [x] `cargo fmt --check` 干净

#### 测试纪录

- 2026-10-10：`cargo test`（lib 175 过 / cli 14 过 / decode_fixture 1 过 / unified_schema 5 过）。
- 2026-10-10：`npm run test:unit` 42 过 0 败（含 dictionaries.test.js 3 项）。

#### 完成状态

- [x] 已完成

### Phase 3 — 验证与收口

#### Task checklist

- [x] `cargo build --release --locked` 通过
- [x] server 冒烟：`/api/health`、`/api/stats`、`/api/cache` 行为不变
- [x] requirement.md 追加 v5.0.0 节；TODO.md / feature.md 创建
- [x] `scripts/check-plan-requirement.sh` 覆盖确认

#### 验收条件

- [x] 全部命令验证通过且结果记录于本文件

#### 测试纪录

- 2026-10-10：`cargo build --release --locked` 通过（Cargo.lock 同步 5.0.0）。
- 2026-10-10：server 冒烟（127.0.0.1:18099）：`/api/health` 200（`version:"5.0.0"`）、`/api/stats` 200、`/api/cache` 200，SQLite 读写正常。
- 2026-10-10：`check-plan-requirement.sh --base HEAD` 通过（需 GIT_CONFIG 注入 safe.directory）。

#### 完成状态

- [x] 已完成

## 整体验收条件

- [x] SQLite 全部读写经 spawn_blocking offload，锁守卫不跨 await；WAL + busy_timeout 生效。
- [x] release 构建恢复 unwind；server 单请求 panic 不再杀进程。
- [x] service 拆分为 5 个子模块，公共 API 与行为不变（全量测试绿）。
- [x] model/error/i18n 单测落地。
- [x] 流程文件齐备（requirement.md v5.0.0 + 本文件 + TODO.md + feature.md）。

## 整体测试紀錄

- `NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost cargo test` — 2026-10-10 全绿。
- `cd frontend && npm run test:unit` — 2026-10-10 42/42。
- `cargo fmt --check` — 2026-10-10 干净。

## 整体完成狀態

- [x] 已完成（Phase 3 server 冒烟为收尾动作，完成后更新）
