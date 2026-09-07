# Unified Lyric Schema Changelog

所有对 `schema/unified-lyric.schema.json` 与 `UnifiedLyric` 序列化契约的变更必须在本文件登记。变更须同步更新以下三处，CI 测试会强制一致性：

1. `src/model.rs` 的 `UNIFIED_LYRIC_SCHEMA_VERSION` 常量
2. `schema/unified-lyric.schema.json` 的 `properties.schema_version.const`
3. `tests/fixtures/unified/*.json` 的 `schema_version` 字段

## 版本升级规则

遵循 semver 精神，但作用于 JSON 契约而非代码 API：

- **主版本（1.x → 2.0）— 破坏性变更**：删除或重命名字段、收窄字段类型、改变字段语义、修改必填集合。客户端遇到更高主版本时应降级到保守展示（见 `docs/unified-json.md` 的 client parsing strategy）。
- **次版本（1.0 → 1.1）— 向后兼容新增**：新增可选字段、新增枚举值、放宽校验约束。老客户端应忽略未知字段继续工作。
- **修订级变更（快照刷新）**：不改契约、仅刷新 golden snapshot（如格式化差异），在「快照刷新」小节登记即可，不提升版本。

## Golden snapshot 工作流

- 快照位于 `tests/fixtures/golden/*.snap`，固化 `UnifiedLyric` 的序列化输出（字段顺序、空集合省略、版本常量）。
- 意图内的序列化变更：`UPDATE_GOLDEN=1 cargo test --test unified_schema golden_serialization` 重新生成，在 changelog 登记后随同提交。
- 意图外的序列化漂移：测试失败即拦截，防止客户端契约被静默破坏。

## 1.0 — 2026-06-06

初始版本。随 v1.6.0（requirement.md）引入：

- 顶层：`schema_version`、`meta`、`mode`（`tracks` | `inline`）、`tracks`、`inline_lines`、`source_refs`、`score`、`cache_refs`、`warnings`、`annotations`。
- `annotations` 覆盖 QQ 音乐助唱标注（`stress`、`breath`、`long_tone`、`portamento_up`、`portamento_down`）。
- `ruby` 采用 UTF-8 字符索引（非字节偏移）。
- 缺失 `schema_version` 的历史 payload 反序列化时默认补 `1.0`（向后兼容）。

## 快照刷新

- 2026-09-07：建立 golden snapshot 基线（`tests/fixtures/golden/`，4 个 fixture），未改契约。
