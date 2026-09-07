# 2026-09-07 — Provider/Decoder 插件化架构规划

## 背景

- 来自完成度复查报告长期优化第 2 项：把 provider metadata、rate-limit、健康度与能力宣告抽象化，未来可用 plugin 方式加入新来源或私有来源。
- 现有基础：`ProviderManifest`/`ProviderCapabilities`/`ProviderConfig` 与 `builtin_provider_registry()` 已存在并被 `/api/sources` 消费；`docs/provider-plugin-manifest.md` 有 v1 draft（仅 manifest 形状，无架构设计）。
- 用户确认范围：**设计文档 + 落地路线图**（不写运行时代码）；边界为 **Provider + Decoder 双轨**。
- 本计划对应 `requirement.md` 的 `v3.0.0 — 2026-09-07 — Provider/Decoder 插件化架构规划`。

## 目标

- [ ] 产出插件化完整设计文档（`docs/provider-plugin-manifest.md` v2）：现状盘点、目标架构（`ProviderPlugin`/`DecoderPlugin` trait + 注册表）、manifest v2 扩展、安全模型。
- [ ] 产出四个里程碑（M1 注册表化 → M2 manifest 驱动 → M3 外部 provider 插件 → M4 外部 decoder 插件）的落地路线图，每个含任务清单、验收标准、规模估计、依赖关系。
- [ ] 明确风险缓解与非目标，供后续批次开工前评审。

## 非目标

- 本轮不实现任何运行时代码（trait、registry、WASM、加载器均属后续里程碑）。
- 不改变现有 `/api/sources` 输出、CLI 行为或 Unified JSON Schema。
- 不做热重载、插件市场、自动更新（永久非目标，已写入设计文档）。

## 风险

- 风险 1：设计文档与后续实作脱节。缓解：每个里程碑开工前重读本文档对应节；M2/M3 设有「实作前评审」决策点。
- 风险 2：WASM 技术选型变动推翻 M3 设计。缓解：路线图明确 spike 先行，spike 结论只影响 M3/M4 技术路线，不动 M1/M2 架构。

## 阶段列表

### Phase 1 — 现状盘点与设计

#### Task checklist

- [x] 盘点现有架构：`Source` 枚举（24 变体）+ match 散落（cli_name/display_name/capabilities/auth/decoder_output_format/provider_config/provider_for_with_options）、`LyricProvider` trait、`ProviderRuntime`（限流/重试）、decoder 封闭 match（8 格式）。
- [x] 确认用户范围：设计文档 + 路线图；Provider + Decoder 双轨。
- [x] 设计核心抽象：`ProviderPlugin`/`DecoderPlugin` trait、字符串键注册表、manifest v2 扩展（runtime/permissions/entry）。
- [x] 设计安全模型：默认关闭、显式 opt-in、权限宿主强制、凭据注入、崩溃隔离。

#### 验收条件

- [x] 设计覆盖现状痛点（新增来源必须改核心枚举、decoder 无 registry、声明与实现耦合）。
- [x] 每个里程碑有可验证的验收标准。

#### 测试纪录

- 不适用（纯设计文档）。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

### Phase 2 — 路线图与文档落地

#### Task checklist

- [x] `docs/provider-plugin-manifest.md` v2 全量重写（含 v1 字段继承表）。
- [x] 四里程碑路线图：M1 注册表化（行为不变重构）→ M2 manifest 驱动配置 → M3 外部 provider（WASM 优先）→ M4 外部 decoder + 治理收口。
- [x] 风险表、非目标、与其他长期项关系（类型生成、部署安全模型）。
- [x] `requirement.md` v3.0.0 追加。

#### 验收条件

- [x] 文档自成体系：新维护者可仅凭文档理解架构意图与开工顺序。
- [x] v1 draft 的 manifest 字段全部有明确处置（继承/新增），无静默丢弃。

#### 测试纪录

- 不适用（纯设计文档）。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

### Phase 3 — 验收与收尾

#### Task checklist

- [x] 文档与 requirement 交叉引用一致。
- [x] 无代码变更，无需运行测试（`git status` 确认仅文档变更）。
- [x] 提交。

#### 验收条件

- [x] 提交历史清晰；plan/requirement 同步。

#### 测试纪录

- `git status --short`：仅 `docs/`、`.plan/`、`requirement.md` 变更。

#### 完成状态

- [ ] 未开始
- [ ] 進行中
- [x] 已完成

## 整体验收条件

- [x] 设计文档 + 路线图产出并提交
- [x] 用户确认的范围（设计文档、双轨）被遵守
- [x] `requirement.md` 已追加 v3.0.0
- [x] 无运行时代码变更

## 整体测试纪录

| 日期 | 指令 / 检查 | 結果 | 備註 |
|------|-------------|------|------|
| 2026-09-07 | `git status --short` | 仅文档变更 | 无代码回归面 |

## 整体完成状态

- [ ] 未開始
- [ ] 進行中
- [x] 已完成
- [ ] 已取消
