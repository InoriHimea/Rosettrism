# Provider / Decoder 插件化规划（v2 设计）

> 状态：设计定稿，未开始实作。本文件取代 2026-06 的 draft（v1），v1 的 manifest 字段形状全部保留。
> 关联：`requirement.md` v3.0.0；`.plan/2026-09-07-plugin-architecture-plan.md`。

## 1. 背景与现状

### 1.1 已有的基础

插件化不是从零开始。以下设施已存在并被生产消费：

| 设施 | 位置 | 状态 |
|---|---|---|
| `ProviderManifest`（manifest_version/source/capabilities/auth/decoder_output_format/config） | `src/provider/mod.rs` | 已被 `/api/sources` 消费 |
| `builtin_provider_registry()` | `src/provider/mod.rs` | 从 `Source` 枚举生成全部 24 个内置 provider 的 manifest |
| `ProviderCapabilities` 声明式 builder | `src/provider/mod.rs` | search/direct_id/word_timing/translation/romanized/ruby/requires_cookie/experimental |
| `ProviderConfig`（timeout/retry/rate_limit） + `ProviderRequestPolicy` | `src/provider/mod.rs` + `runtime.rs` | 已驱动运行时限流与重试 |
| `LyricProvider` trait | `src/provider/mod.rs` | 统一 search/fetch 接口 |
| `InputFormat` 枚举 + `decode_bytes`/`decode_raw_bytes` | `src/decoder/mod.rs` | 8 种格式硬编码 match 分发 |

### 1.2 痛点

1. **新增 provider 必须改核心枚举**：`Source` 是 `Copy + ValueEnum` 编译期枚举，每加一个来源要动 `mod.rs` 的多处 match（cli_name/display_name/capabilities/auth/decoder_output_format/provider_config/provider_for_with_options）。私有来源无法在不 fork 的前提下接入。
2. **decoder 无 registry**：`decode_bytes` 是封闭 match，新增格式同样要改核心代码。
3. **能力声明与实现耦合**：capabilities 分散在 match 臂中，无法独立审计。
4. **无法在运行时组合**：cache/service/server 都以 `Source` 为键，外部来源没有一等公民身份。

## 2. 设计原则

1. **渐进演进，不重写**：分四个里程碑，每个都可独立发布、可回滚、不破坏现有 API（`/api/sources` 输出形状保持兼容）。
2. **Manifest 是唯一事实源**：capabilities/config/auth 声明与运行时行为由同一结构驱动，消除双轨漂移。
3. **内置与外部同构**：内置 provider 就是一个编译进二进制的"插件"；外部插件与它走完全相同的注册与调用路径。不搞两套特权。
4. **安全默认**：外部插件默认关闭；必须显式 opt-in（目录 + allowlist）；网络与文件权限在 manifest 声明、在运行时强制。
5. **契约不锁死实现技术**：trait 抽象先落地；WASM/子进程等动态加载技术作为后续里程碑的可选项，不进入第一版契约。

## 3. 目标架构

### 3.1 核心抽象：`ProviderPlugin` trait（取代散落的 match）

```rust
/// 一个 provider 插件 = manifest 声明 + 能力构造 + provider 工厂。
pub trait ProviderPlugin: Send + Sync {
    fn manifest(&self) -> &ProviderManifest;
    /// 凭据解析后的 provider 实例工厂。
    fn create_provider(
        &self,
        credential: Option<&ProviderCredential>,
        options: &ProviderOptions,
    ) -> Result<Box<dyn LyricProvider>>;
}
```

- `Source` 枚举**保留**（CLI/API 兼容），但每个变体获得一个 `ProviderPlugin` 实现单元（同文件或独立文件），match 臂收敛为「注册表查找」。
- `builtin_provider_registry()` 的语义升级：从「枚举遍历」变为「内置插件注册表导出 manifest」。输出形状不变。

### 3.2 核心抽象：`DecoderPlugin` trait（新增）

```rust
/// 一个 decoder 插件 = 格式声明 + 解码入口。
pub trait DecoderPlugin: Send + Sync {
    /// 稳定格式标识（对应现 InputFormat 变体，或外部格式的 snake_case 名）。
    fn format_id(&self) -> &'static str;
    /// 字节特征嗅探（可选）；返回置信度权重。
    fn detect(&self, bytes: &[u8]) -> Option<u32>;
    /// 解码为 LyricDocument。
    fn decode(&self, bytes: &[u8]) -> Result<LyricDocument>;
    /// 原始解码（如 QRC/KRC 的解密直出）；默认实现返回原文。
    fn decode_raw(&self, bytes: &[u8]) -> Result<Vec<u8>> { Ok(bytes.to_vec()) }
}
```

- `decode_bytes`/`decode_raw_bytes` 的 match 分发替换为 `DecoderRegistry::lookup(format_id)` 或嗅探打分。
- `InputFormat` 枚举保留为 CLI 兼容层，新增 `format_id: String` 的运行时形式；`detect.rs` 的启发式改为对注册表逐个打分。

### 3.3 注册表与查找

```rust
pub struct ProviderRegistry {
    plugins: Vec<Box<dyn ProviderPlugin>>,   // 内置 + 外部
    by_source_name: HashMap<String, usize>, // source_name -> 插件槽位
}

pub struct DecoderRegistry {
    plugins: Vec<Box<dyn DecoderPlugin>>,
    by_format_id: HashMap<String, usize>,
}
```

- 查找键统一为 **字符串 `source_name` / `format_id`**（与 manifest 一致），不再依赖枚举穷举。
- `service.rs` 中 `source_from_cli_name` → `registry.lookup(name)`；`server.rs` 的 `/api/sources` 直接导出两个注册表。

### 3.4 外部插件形态（里程碑 M3）

- 目录约定：`<data_dir>/plugins/providers/<name>/rosettrism-provider.json`（manifest）、同名 `.wasm` 或由 manifest 指向的可执行/脚本。
- manifest v2 在 v1 基础上新增（全部可选，缺省即安全值）：
  - `"runtime": "builtin" | "wasm" | "process"`——执行模型。
  - `"permissions": { "network": ["host-pattern"...], "fs_read": [...], "fs_write": [...] }`——能力边界，运行时强制。
  - `"entry": "main.wasm"`——runtime 入口。
- 加载规则：
  1. 默认只加载 `builtin`。
  2. 环境变量 `ROSETTRISM_PLUGIN_DIR` 指向插件目录时，逐一读取 manifest、校验 `manifest_version` 与必要字段。
  3. `permissions` 超出全局 allowlist 的插件拒绝加载，错误进入 `/api/providers/health` 的诊断输出。
  4. 外部插件的能力声明**不可信**：`capabilities` 只作展示；实际能力以运行时探测/首次调用结果为准（`experimental` 强制为 true，直到健康度达标）。

### 3.5 安全模型（M3 前置设计，M4 收口）

| 维度 | 策略 |
|---|---|
| 加载 | 默认关闭；`ROSETTRISM_PLUGIN_DIR` 显式 opt-in |
| 网络 | manifest 声明 host-pattern allowlist；WASM 场景由 host 函数注入受控 fetch；process 场景由代理/系统层限制 |
| 文件 | 默认无 fs 权限；`fs_read`/`fs_write` 显式声明路径前缀 |
| 凭据 | 插件永不直接读环境变量/cookie 文件；凭据由宿主解析后作为参数注入 |
| 超时/限流 | 复用 `ProviderRequestPolicy`，对插件无差别强制 |
| 崩溃隔离 | WASM trap / 子进程退出 → 映射为该 provider 的 `provider_warning`，不影响其他 provider |
| 沙箱逃逸面 | WASM 无系统调用面（wasi 限定）；process 模式文档化为「信任边界=本机用户」，不承诺强隔离 |

## 4. 落地路线图（四个里程碑）

### M1 — 注册表化重构（纯内部，无新功能）

**目标**：消除 match 散落，建立 `ProviderPlugin`/`DecoderPlugin` trait 与注册表；全部行为不变。

- [ ] 定义 `ProviderPlugin`、`DecoderPlugin` trait 与两个 Registry。
- [ ] 24 个内置 provider 迁移为插件实现单元（每 provider 一个文件或 mod 内一个 impl 块）。
- [ ] 8 种 decoder 迁移为 `DecoderPlugin` 实现；`detect.rs` 改为嗅探打分。
- [ ] `provider_for_with_options`、`decode_bytes`、`decode_raw_bytes` 改为注册表查找；旧签名保留为薄包装。
- [ ] 回归：`/api/sources`、CLI、全部测试不变。

**验收**：现有测试全绿；`cargo test --lib provider::` 无行为 diff；`/api/sources` JSON 逐字段一致。

**规模**：约 8–12 个文件中大型重构；1–2 个开发批次。

### M2 — Manifest 驱动配置（内置插件声明外置）

**目标**：内置插件的 capabilities/config 声明从 Rust match 移到**编译期校验的 manifest 数据**（仍编译进二进制，如 `include_str!` + 构建期 schema 校验），UI/CLI 可全量消费。

- [ ] `rosettrism-provider.json` v2 schema 定稿（含 runtime/permissions/entry 可选字段）。
- [ ] 内置插件 manifest 外置为 JSON（构建期校验：字段齐全、与 `Source` 枚举对齐、与 Rust 侧工厂存在性对齐）。
- [ ] `/api/sources`、`/api/providers/health` 输出增加 `runtime`/`permissions` 字段（缺省 builtin/空）。
- [ ] 决策点（实作前评审）：manifest 外置的收益（声明可审计、与外部插件同构）vs 成本（构建期校验脚本、双源同步）。

**验收**：构建期校验失败即 CI 失败；`/api/sources` 新字段向后兼容（纯新增）。

**规模**：中等；1 个开发批次。

### M3 — 外部 provider 插件（WASM 优先）

**目标**：第三方可在不改核心代码的前提下接入私有来源。

- [ ] WASM 运行时选型（wasmtime vs wasmi；评估 GC/async/宿主函数需求）。
- [ ] `LyricProvider` 的 WIT 或手工 ABI 定义：search/fetch 两入口 + 受控 fetch 宿主函数。
- [ ] manifest v2 加载器 + 权限执行（host-pattern 网络注入）。
- [ ] 一个参考实现插件（如针对一个公开 web 来源的完整 WASM 插件）作为契约测试。
- [ ] 错误映射：WASM trap → `provider_warning`；权限拒绝 → 明确诊断码。

**验收**：参考插件从 `ROSETTRISM_PLUGIN_DIR` 加载、通过权限校验、完成 search+fetch、出现在 `/api/sources`；无权限的网络请求被拒绝且诊断可读。

**规模**：大；2–3 个开发批次，需先做技术验证 spike。

### M4 — 外部 decoder 插件 + 治理收口

**目标**：解码格式同样可插拔；安全与文档收口。

- [ ] `DecoderPlugin` 的 WASM ABI（decode/decode_raw/detect）。
- [ ] 嗅探冲突策略：多个插件可 detect 时按置信度+注册顺序裁决，诊断输出记录裁决。
- [ ] 插件签名与分发建议（社区插件目录约定、版本 pin）。
- [ ] 安全模型文档化 + 部署指南（与部署安全模型长期项衔接）。

**验收**：外部 decoder 插件可加载并参与 `decode_bytes` 分发；嗅探裁决有诊断；安全文档评审通过。

**规模**：中–大；1–2 个开发批次。

### 里程碑依赖与发布策略

```
M1 ──→ M2 ──→ M3 ──→ M4
```

- 每个 M 独立发布（对应 requirement 版本节），M1/M2 不引入任何用户可见变化（除 `/api/sources` 纯新增字段）。
- M3 之前完成部署安全模型中的 CORS/反代部分更佳（插件网络权限与部署边界相互约束）。
- WASM 选型 spike（1–2 天）应排在 M3 承诺之前；spike 结论可能调整 M3/M4 的技术路线，但不动 M1/M2。

## 5. 风险与缓解

| 风险 | 等级 | 缓解 |
|---|---|---|
| M1 重构引入行为回归 | 高 | 验收锚定「`/api/sources` 逐字段一致 + 全量测试」；分 provider 小步迁移 |
| WASM ABI 设计过早锁死 | 中 | M1/M2 不含 ABI；spike 先行；M3 落地前用参考插件做契约测试 |
| 外部插件拖慢启动/泄漏资源 | 中 | 注册表惰性实例化；插件实例有 Drop 钩子；健康度统计暴露插件级指标 |
| manifest 双源漂移（JSON vs Rust 工厂） | 中 | M2 构建期校验 + 失败即 CI 红 |
| 安全声明的实现漏洞（网络越权） | 高 | 权限由宿主注入层强制，非插件自觉；参考插件含越权负向测试 |

## 6. 非目标

- 不做热重载（进程重启加载插件即可）。
- 不做插件市场/自动更新（只约定目录与签名建议）。
- 不在本设计内改变 Unified JSON Schema 或歌词聚合语义。
- 不承诺 process 模式的强沙箱隔离（文档化为信任边界）。
- 不移除 `Source`/`InputFormat` 枚举（CLI/API 兼容层永久保留）。

## 7. 与其他长期项的关系

- **多客户端类型生成**：插件 manifest schema 也应进入类型生成范围（M2 之后）。
- **部署安全模型**：插件网络权限与 CORS allowlist/反代模板应统一设计（M3 前评审）。
- **Phase 7 媒体时钟**：无直接依赖；插件化不触碰音频 URL 前置条件。

## 附：v1 draft 的字段继承表

| v1 字段 | v2 处置 |
|---|---|
| `manifest_version` | 保留，恒为 1（v2 是文档版本，非 manifest 版本） |
| `source`/`name`/`source_name`/`display_name` | 保留 |
| `capabilities`（8 字段） | 保留；外部插件声明不可信，运行时强制 experimental |
| `auth` | 保留 |
| `decoder_output_format` | 保留；指向 `format_id` 字符串 |
| `config`（timeout/retry/rate_limit） | 保留；宿主对插件无差别强制 |
| —（新增） | `runtime`、`permissions`、`entry`（全部可选，缺省安全值） |
