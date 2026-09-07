# 2026-09-07 — Unified JSON Schema 兼容性治理

## 背景

- 来自完成度复查报告中期优化第 4 项：新增 schema changelog、fixture golden snapshots 与主/次版本升级规则，让客户端能更可靠地升级。
- 现状：schema v1.0、4 个 fixture、手写 validator + model 双轨验证（v1.6.0 引入），但缺 changelog、缺 golden snapshot（fixture 变更无告警）、缺三方版本常量一致性断言。
- 本计划对应 `requirement.md` 的 `v2.5.0 — 2026-09-07 — Unified JSON Schema 兼容性治理`。

## 目标

- [ ] 新增 `SCHEMA_CHANGELOG.md`，含主/次版本升级规则、golden snapshot 工作流、1.0 基线条目。
- [ ] 新增 golden snapshot 测试：固化 `UnifiedLyric` 序列化输出（字段顺序、空集合省略、版本常量）。
- [ ] 新增三方一致性测试：model 常量 = schema 文件 const = 全部 fixture 值，捕获单边漂移。
- [ ] 在 `docs/unified-json.md` 补 Schema governance 说明，指向 changelog。
- [ ] 全部既有测试保持通过。

## 非目标

- 不升级 schema 主/次版本（本轮保持 1.0，仅建立治理基线）。
- 不修改 schema 字段定义或 model 结构。
- 不引入 JSON Schema 第三方 validator crate（沿用仓库内置手写 validator）。

## 风险

- 风险 1：golden snapshot 跨平台行尾差异导致 CI 假阳性。缓解：snapshot 内容为序列化 JSON 字符串，不含操作系统行尾；`UPDATE_GOLDEN=1` 随同提交。
- 风险 2：三方一致性测试对“意图内升级”造成摩擦。缓解：测试报错信息明确指向 changelog 与 `UPDATE_GOLDEN=1`，升级路径已文档化。

## 阶段列表

### Phase 1 — 需求釐清与设计

#### Task checklist

- [x] 盘点 schema 现状（v1.0、4 fixture、手写 validator、无 changelog、无 golden）。
- [x] 确认治理设计：changelog + golden snapshot + 三方一致性断言 + 文档章节。
- [x] 确认不引入第三方 validator 依赖。

#### 验收条件

- [x] 设计方向已记录。
- [x] 非目标与风险已确认。

#### 测试纪录

- 尚未执行。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

### Phase 2 — 實作

#### Task checklist

- [x] `SCHEMA_CHANGELOG.md`：主/次版本规则、golden 工作流、1.0 基线条目、2026-09-07 快照刷新记录。
- [x] `tests/unified_schema.rs`：`golden_serialization_snapshots_match_model_round_trip`，4 个 fixture 的序列化 golden。
- [x] `tests/unified_schema.rs`：`schema_version_constant_matches_schema_file_and_fixtures`，三方一致性断言。
- [x] `docs/unified-json.md`：新增 Schema governance 章节指向 changelog。
- [x] 新增测试均通过；回归验证确认 binding 测试能捕获单边漂移。

#### 验收条件

- [x] golden snapshot 在 4 个 fixture 上 round-trip 通过。
- [x] 三方一致性测试在 model const 漂移时失败并指向 changelog。
- [x] 与非目标无冲突。

#### 测试纪录

- `UPDATE_GOLDEN=1 cargo test --test unified_schema golden_serialization` → 生成 4 个 .snap。
- `cargo test --test unified_schema` → 5 passed。
- 临时改 model const 为 `1.0-BREAK-TEST` → binding 测试 FAILED 并提示「update both together and record the change in SCHEMA_CHANGELOG.md」。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

### Phase 3 — 验收与收尾

#### Task checklist

- [x] 运行完整 Rust 测试与格式检查。
- [x] 更新 `requirement.md` v2.5.0 状态。
- [x] 版本号：治理基线属 patch 级，维持 4.13.0，不打新 tag。

#### 验收条件

- [x] 所有必要检查已通过。
- [x] 文件已同步更新。

#### 测试纪录

- `cargo test --test unified_schema` → 5 passed。
- `cargo test --test cli --test decode_fixture` → 14 + 1 passed。
- `cargo test --lib -- --test-threads=1 server::tests` → 23 passed。
- `cargo fmt --check` → clean；`git diff --check` → clean。
- 全量并行跑 lib 测试时 17 条 wiremock provider 测试因 Windows 端口耗尽（`AddrInUse`）间歇失败，单跑或串行即通过；属已知环境限制（v1.8.0 记录），非本次回归——baseline 单跑同一测试亦通过。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

## 整体验收条件

- [x] 需求目标已完成
- [x] 测试纪录完整
- [x] 相关文件已同步
- [x] `requirement.md` 已追加

## 整体测试纪录

| 日期 | 指令 / 检查 | 结果 | 備註 |
|------|-------------|------|------|
| 2026-09-07 | `UPDATE_GOLDEN=1 cargo test --test unified_schema golden_serialization` | 生成 4 个 .snap | 基线建立 |
| 2026-09-07 | `cargo test --test unified_schema` | 5 passed | 含 golden + 三方一致性 |
| 2026-09-07 | binding 回归验证（临时改 const） | FAILED → 提示 changelog | 确认能捕获漂移 |
| 2026-09-07 | `cargo test --test cli --test decode_fixture` | 14 + 1 passed | 无回归 |
| 2026-09-07 | `cargo test --lib -- --test-threads=1 server::tests` | 23 passed | 无回归 |
| 2026-09-07 | `cargo fmt --check` / `git diff --check` | clean | — |

## 整体完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成
- [ ] 已取消
