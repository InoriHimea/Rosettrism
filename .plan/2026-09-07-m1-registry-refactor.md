# 2026-09-07 — M1 注册表化重构

## 背景

- 落地 `docs/provider-plugin-manifest.md` §3 路线图 M1：建立 `ProviderPlugin`/`DecoderPlugin` trait 与注册表，把散落的 match 臂收敛为注册表查找；全部行为不变。
- 本计划对应 `requirement.md` 的 `v3.1.0 — 2026-09-07 — M1 注册表化重构`。

## 目标

- [ ] `DecoderPlugin` trait + `DecoderRegistry`：8 种内置格式迁移为插件实现；`decode_bytes`/`decode_raw_bytes` 改为注册表分发，旧签名保留。
- [ ] `ProviderPlugin` trait + `ProviderRegistry`：24 个内置 provider 通过薄包装注册；`builtin_provider_registry()` 改走注册表导出，输出逐字段不变。
- [ ] 不破坏 `/api/sources`、CLI、Unified JSON Schema。
- [ ] 现有测试全绿（wiremock flaky 视为环境限制，非回归）。

## 非目标

- 不迁移 `provider_for_with_options` 的 match 主体（保留至 M2，薄包装够用）。
- 不改 manifest 字段或 Unified JSON Schema。
- 不引入 WASM/外部进程加载。

## 风险

- 风险 1：嗅探优先级改变导致 Auto 检测漂移。缓解：registry 内 `detect` 用置信度打分，复刻旧 detect.rs 优先级（TTML=95 > XML-QRC=90 > 文本头 QRC=80）；新增 `detect_matches_legacy_detector_on_qrc_text_header` 测试固化。
- 风险 2：大规模 match 迁移引入隐藏回归。缓解：provider 侧用薄包装委托而非重写构造逻辑；decoder 侧逐格式迁移后立刻跑 decode_fixture/cli 验证。

## 阶段列表

### Phase 1 — Decoder 注册表

#### Task checklist

- [x] `DecoderPlugin` trait（format_id/detect/decode/decode_raw）。
- [x] `DecoderRegistry`（lookup/detect/builtin）。
- [x] 7 个内置 decoder 插件（apple_music/json/krc/qrc/text/yrc/lrc）。
- [x] `decode_bytes`/`decode_raw_bytes` 改为 registry 分发；旧签名保留。
- [x] 嗅探优先级复刻旧 detect.rs（TTML > XML-QRC > 文本头 QRC）。
- [x] registry 行为测试 5 条。

#### 验收条件

- [x] `cargo test --lib decoder` → 43 passed。
- [x] `cargo test --test decode_fixture --test cli` → 15 passed（含 sample.qrc 文本头嗅探与 TTML）。

#### 测试纪录

- `cargo test --lib decoder` → 43 passed。
- `cargo test --test decode_fixture` → 1 passed。
- `cargo test --test cli` → 14 passed。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

### Phase 2 — Provider 注册表

#### Task checklist

- [x] `ProviderPlugin` trait（manifest/create_provider）。
- [x] `ProviderRegistry`（lookup/manifests/builtin）。
- [x] `BuiltinSourcePlugin` 薄包装：manifest 委托 `Source::manifest()`，create_provider 委托 `provider_for_with_options`。
- [x] `builtin_provider_registry()` 改走 `ProviderRegistry::builtin().manifests()`。
- [x] `ProviderManifest` 加 `PartialEq, Eq`（支持 registry 测试断言）。
- [x] registry 行为测试 4 条。

#### 验收条件

- [x] `provider_registry_manifests_equal_legacy_registry_output`：注册表导出与旧枚举遍历逐条相等。
- [x] `provider_registry_covers_all_sources_and_manifests_match_legacy`：每个 Source 在 registry 有插件且 manifest 一致。
- [x] `cargo test --lib provider::tests` → 10 passed（含原有 metadata/gating + 新增 4 条 registry）。

#### 测试纪录

- `cargo test --lib metadata_registry` → 1 passed。
- `cargo test --lib provider_registry` → 3 passed。
- `cargo test --lib duplicate_source` → 1 passed。
- `cargo test --lib lookup_rejects` → 1 passed。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

### Phase 3 — 回归验证与收尾

#### Task checklist

- [x] `cargo fmt --check` + `git diff --check` clean。
- [x] 分组回归：decoder 43、cache 17、service 9、server 23、provider::tests 10、unified 7 全过。
- [x] baseline 对比：HEAD worktree 与 M1 worktree 的 `server::tests` 均 23 passed，行为一致。
- [x] wiremock flaky 记录：串行跑 31 个 wiremock provider 测试因 Windows 端口/缓冲耗尽（`tcp bind local error 10055`/`AddrInUse`）失败；单跑或冷却后全过，属已知环境限制（v1.8.0 记录），非本次回归。
- [x] 更新 `requirement.md` v3.1.0 状态。

#### 验收条件

- [x] 无真实回归（非 flaky 测试全绿）。
- [x] 文件已同步更新。

#### 测试纪录

- `cargo fmt --check` → clean。
- `cargo test --lib decoder/cache/service/server/provider::tests/unified` → 全过。
- `cargo test --test decode_fixture --test cli --test unified_schema` → 全过。
- baseline (HEAD) 与 M1 的 `server::tests` 均 23 passed。
- wiremock 串行 flaky：utaten/spotify/lrclib 等 31 条，单跑全过。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

## 整体验收条件

- [x] Decoder/Provider 双注册表落地，行为不变
- [x] `/api/sources` 经 `api_routes_return_json_with_cache_enabled` 断言通过（provider_health[0].status == healthy、stats.cache.upstream_entries 等）
- [x] 测试纪录完整
- [x] 相关文件已同步
- [x] `requirement.md` 已追加

## 整体测试纪录

| 日期 | 指令 / 检查 | 结果 | 備註 |
|------|-------------|------|------|
| 2026-09-07 | `cargo build` | clean | 无 warning（除增量缓存硬链接） |
| 2026-09-07 | `cargo test --lib decoder` | 43 passed | 含 5 条新 registry 测试 |
| 2026-09-07 | `cargo test --test decode_fixture --test cli` | 1 + 14 passed | 嗅探优先级修复 |
| 2026-09-07 | `cargo test --lib provider::tests` | 10 passed | 含 4 条新 registry 测试 |
| 2026-09-07 | `cargo test --lib server/cache/service/unified` | 23/17/9/7 passed | 无回归 |
| 2026-09-07 | baseline (HEAD) `server::tests` | 23 passed | 行为一致对比 |
| 2026-09-07 | wiremock 串行 | 31 flaky | 端口/缓冲耗尽，单跑全过 |
| 2026-09-07 | `cargo fmt --check` / `git diff --check` | clean | — |

## 整体完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成
- [ ] 已取消
