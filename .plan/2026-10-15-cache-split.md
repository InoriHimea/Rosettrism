# 2026-10-15 — cache.rs 拆分

## 背景
- `src/cache.rs` 达 1935 行，类型、四张表的读写、迁移与维护操作混在一起。见 `requirement.md` v5.0.2。

## 目标
- [x] 按表域拆为 `src/cache/{mod,types,upstream,unified,ai_scores,fetch_runs,maintenance,tests}.rs`
- [x] 公共 API 不变（`pub use types::*`）
- [x] `.gitattributes` 固定 golden 快照 LF

## 非目标
- 不改 SQL、schema 迁移或 async/`spawn_blocking` 语义。

## 风险
- 私有方法跨文件可见性：各子模块 impl 均为 `pub(super)`，`cargo check --tests` 验证。
- 拆分用脚本按函数名分组，已核对测试数不变（5 项）与全量测试结果。

## 阶段列表

### Phase 1 — 拆分与验证
- [x] 脚本分组拆分、`cargo fmt`
- [x] `cargo test` 全绿

#### 测试紀錄
- lib 171 / cli 14 / decode_fixture 1 / unified_schema 5 全部通过。

#### 完成状态
- [x] 已完成
