# 2026-10-15 — server.rs 拆分

## 背景
- `src/server.rs` 达 2328 行，handlers、搜索过滤、错误类型、静态资源与测试混在一起。见 `requirement.md` v5.0.1。

## 目标
- [x] 按职责拆为 `src/server/{mod,handlers,search,assets,error,tests}.rs`
- [x] 对外 API 与路由行为不变

## 非目标
- 不拆 `cache.rs`（另行立项）。
- 不修改 golden 快照的换行问题。

## 风险
- 可见性遗漏导致编译失败：已通过 `cargo check --tests` 验证。
- 测试数量变化：lib 测试 171 全过。

## 阶段列表

### Phase 1 — 拆分与验证
- [x] 按行范围拆分、调整 `pub(super)` 可见性
- [x] `cargo fmt --check`、`cargo test` 通过

#### 测试紀錄
- lib 171 / cli 14 / decode_fixture 1 通过；unified_schema golden 因 CRLF 检出失败（既有问题，转 LF 后 5/5 通过）。

#### 完成状态
- [x] 已完成
