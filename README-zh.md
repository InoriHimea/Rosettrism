# Rosettrism

[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![Frontend](https://img.shields.io/badge/Dashboard-React%20%2B%20Vite-61dafb.svg)](frontend/package.json)
[![Schema](https://img.shields.io/badge/Unified%20JSON-1.0-blue.svg)](schema/unified-lyric.schema.json)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPLv3-blue.svg)](LICENSE)

[English](README.md) · [Unified JSON 指南](docs/unified-json.md) · [完成度複查報告](docs/completion-audit-2026-06-06.md)

Rosettrism 是一个 Rust 单二进制歌词工具集，可解码本地歌词文件、从在线来源获取歌词、缓存上游请求，并把多个候选聚合为稳定的统一 JSON。项目同时提供 CLI、本地 Axum HTTP API 与内嵌 React 仪表盘。

## 项目状态

上一轮计划已经完成并完成复查。Rosettrism 目前可以作为 CLI / Server / Dashboard 使用，但项目不应视为完全结束，仍然建议继续做功能迭代与优化。下一阶段最值得投入的是可靠性与可维护性：API 端到端测试、统一 API 错误码、Provider 健康度统计、缓存维护工具、AI 评分回放、Schema 兼容治理，以及仪表盘体验打磨。

详细完成矩阵与后续建议见 [docs/completion-audit-2026-06-06.md](docs/completion-audit-2026-06-06.md)。

## 功能特性

- **本地解码**：KRC、QQ QRC/XML、网易云 YRC、Apple Music TTML、LRC、纯文本与 Rosettrism JSON。
- **多在线来源**：酷狗、QQ 音乐、网易云、Apple Music、Musixmatch、PetitLyrics、LRCLIB、UtaTen、JOYSOUND、咪咕 H5、LINE MUSIC、KKBOX、Genius、AZLyrics、Songtexte、Uta-Net、J-Lyric、J-Total、Kashinavi、UtaMap、Lyrical Nonsense、Animesongz、AWA、TuneCore、RockLyric、Spotify Lyrics 与 Offline DB。
- **统一歌词模型**：默认输出多轨 JSON，可选 inline 合并行，支持 `schema_version`、助唱标注、翻译、读音、ruby 与罗马音轨道。
- **助唱标注**：QQ 音乐助唱标注可在可用时自动获取，并映射为逐音节、带时间的声乐技巧标记。
- **TTL 缓存**：Provider `search` 与 `fetch` 调用会写入 SQLite，默认 TTL 为 7 天。
- **聚合与 AI 可追踪性**：聚合 fetch 会优先选择高质量 timed / word-timed 歌词；可选 OpenAI-compatible AI 优选会记录 model、endpoint、候选 hash、评分、原因与最终来源。
- **可观测性**：fetch run 会记录近期 query、source、mode、status、message、cache hit/store、provider warning、AI skip 与 no-lyrics 结果。
- **本地仪表盘**：`rosettrism server` 提供内嵌 Dashboard 与 HTTP API。非本地绑定必须设置 `ROSETTRISM_SERVER_TOKEN`。
- **歌词质量诊断**：播放 normalization 会区分逐字同步、逐行同步、无同步时间和异常时间轴，并显式报告逐字、多语、ruby、助唱标注能力及可解释降级原因；缺失时间戳的文本不会被伪装为 0ms 可播放歌词。
- **播放器会话**：真实媒体包装层支持播放队列、顺序/单曲循环/列表循环/随机模式、音量与静音、Media Session 系统控制、错误重试和可信刷新恢复；只有显式标记为 durable 且未过期的音源才允许持久化。

Rosettrism **不实现** CAPTCHA 绕过、凭证收集、SSL pinning 绕过、私有 App 签名或非公开协议自动化。

## 目录

- [安装](#安装)
- [快速开始](#快速开始)
- [CLI 用法](#cli-用法)
- [服务器 API](#服务器-api)
- [Unified JSON](#unified-json)
- [缓存与可观测性](#缓存与可观测性)
- [助唱标注](#助唱标注)
- [来源](#来源)
- [路线图](#路线图)
- [开发](#开发)
- [贡献](#贡献)
- [协议](#协议)

## 安装

### 构建 Rust 二进制

```bash
cargo build --release
```

Release 二进制输出位置：

```text
target/release/rosettrism
```

Windows 输出位置：

```text
target\release\rosettrism.exe
```

### 构建内嵌仪表盘

```bash
cd frontend
npm install
npm run build
```

Rust server 会内嵌 `frontend/dist`。如果前端文件有变化，打包前请重新构建仪表盘。

## 快速开始

解码本地歌词文件：

```bash
rosettrism decode ./lyric.qrc --input-format qrc --format json -o ./lyric.json
```

聚合多个来源并输出统一 JSON：

```bash
rosettrism fetch "歌曲名 歌手" --merge-mode tracks --top 1 -o ./unified.json
```

启动本地服务器与仪表盘：

```bash
rosettrism server --host 127.0.0.1 --port 8080 --open
```

## CLI 用法

### 解码本地文件

```bash
rosettrism decode ./lyric.qrc --input-format qrc --format json -o ./lyric.json
rosettrism decode ./lyric.krc --format lrc -o ./lyric.lrc
```

### 聚合来源

```bash
rosettrism fetch "歌曲名 歌手" --merge-mode tracks --top 1
rosettrism fetch "歌曲名 歌手" --merge-mode inline --top 3 -o ./unified.json
```

强制刷新并覆盖 TTL：

```bash
rosettrism fetch "歌曲名 歌手" --ttl 7d --force-refresh
```

### 指定来源获取

使用 `--source` 做指定来源 fetch 时，请提供 `--format raw` 或 `--format json`。

```bash
rosettrism fetch "歌曲名 歌手" --source lrclib --format json
rosettrism fetch "歌曲名 歌手" --source qq --format raw -o ./qq.raw.txt
```

### 搜索候选

搜索指定来源并保存选中的原始 payload：

```bash
rosettrism search "歌曲名 歌手" --source kugou -o ./lyric.krc
```

不指定来源时，搜索返回聚合候选 JSON：

```bash
rosettrism search "歌曲名 歌手" -o ./candidates.json
```

## 服务器 API

启动本地服务器：

```bash
rosettrism server --host 127.0.0.1 --port 8080 --open
```

绑定非本地地址时必须设置 `ROSETTRISM_SERVER_TOKEN`。客户端需通过 `x-rosettrism-token: <token>` 或 `Authorization: Bearer <token>` 发送该值。缺失或错误时会收到 JSON `401` 响应，例如 `{ "error": "missing or invalid server token" }`。

获取统一 JSON：

```bash
curl -X POST http://127.0.0.1:8080/api/fetch \
  -H "content-type: application/json" \
  -H "x-rosettrism-token: ${ROSETTRISM_SERVER_TOKEN}" \
  -d '{"query":"歌曲名 歌手","merge_mode":"tracks","top":1}'
```

获取来源原始文本：

```bash
curl -X POST http://127.0.0.1:8080/api/fetch \
  -H "content-type: application/json" \
  -H "Authorization: Bearer ${ROSETTRISM_SERVER_TOKEN}" \
  -d '{"query":"歌曲名 歌手","source":"qq","format":"raw"}'
```

Dashboard token 行为：

- 未设置 `ROSETTRISM_SERVER_TOKEN` 的 localhost 服务不需要 Dashboard token。
- 远端服务需要在 Settings 中填写同一个 token。Dashboard 仅保存到 `sessionStorage`，浏览器会话结束后清除；也可以使用 **Clear Token** 立即移除。

可用端点：

- `GET /api/health`
- `GET /api/sources` — 返回内置 provider manifest 注册表，其中包含每个 source 的 timeout/retry/rate-limit 配置。这些 manifest 值会在 provider runtime 中被强制执行，然后才会发起任何上游请求。
- `GET /api/providers/health?limit=20` — 汇总近期 `fetch_runs` 中的 provider 表现。它反映的是 runtime 强制执行后的行为，不是实时探测。
- `POST /api/fetch`
- `GET /api/cache`
- `GET /api/cache/:id`
- `DELETE /api/cache/:id`
- `POST /api/cache/:id/revalidate`
- `GET /api/runs`
- `GET /api/stats`

## Unified JSON

统一聚合输出由 [`schema/unified-lyric.schema.json`](schema/unified-lyric.schema.json) 描述。tracks、inline lines、annotations、ruby、translation、reading 与 romanization 的兼容规则见 [`docs/unified-json.md`](docs/unified-json.md)。

客户端解析器应忽略未知字段，使新版 Rosettrism 可以添加可选数据而不破坏旧客户端。请使用 `schema_version` 做降级策略：兼容 `1.x` payload 时可乐观接收；遇到更新 major 版本时，优先回退到 `tracks[0].document.lines` 或 `inline_lines`。

## 缓存与可观测性

缓存数据库路径优先级：

1. `--db <PATH>`
2. `ROSETTRISM_DB`
3. `LRC_DECODE_DB`
4. 系统数据目录回退

缓存会存储上游原始操作、派生统一结果、fetch-run 记录、可追踪 AI 评分记录与 schema migration。上游缓存键基于 source、operation、规范化请求数据与请求版本；cookie 与 token 不进入缓存键。

fetch-run 可观测性覆盖聚合 fetch、多来源 search、选中结果 fetch 与聚合成员 fetch。Dashboard Overview/Cache 页面与 `/api/runs` 会展示 `provider_warning`、`ai_skipped`、`no_lyrics_found`、`cache_hit`、`cache_store` 等状态。每条 run 会记录 `started_at`、可选 `finished_at`、`duration_ms`、`provider_count`、`candidate_count` 与 `cache_event`；`created_at` 保留为插入时间，方便旧客户端兼容。

Provider health 来自带有具体 `source` 的近期 `fetch_runs`，不是实时探测。`GET /api/providers/health?limit=N` 与 `/api/stats.provider_health` 会汇总每个 provider 最近 N 次 run：成功率、平均耗时、warning/error 比例以及最后一条 warning 或 error。provider runtime 现在会在上游请求前强制执行每个 source 的 manifest timeout、retry、backoff 和 rate-limit 配置。状态定义：近期成功率至少 80%、没有错误且 warning 未升高时为 `healthy`；出现 warning/error 或成功率低于 80% 时为 `degraded`；错误占比过高或成功率低于 50% 时为 `critical`。排查 degraded provider 时，请先查看最后错误、比较 cache hit/store 与上游请求，确认 rate limit 后再用 `--force` 重试，并检查 provider cookie 或地区可用性。

缓存维护命令统一放在 `cache` 子命令组下：

```bash
rosettrism --db /var/lib/rosettrism/cache.sqlite cache stats
rosettrism --db /var/lib/rosettrism/cache.sqlite cache runs --limit 100
rosettrism --db /var/lib/rosettrism/cache.sqlite cache ai-scores --limit 100
rosettrism --db /var/lib/rosettrism/cache.sqlite cache export --format jsonl --output /backup/rosettrism-cache.jsonl
rosettrism --db /var/lib/rosettrism/cache.sqlite cache export --format pretty-json --upstream --unified
rosettrism --db /var/lib/rosettrism/cache.sqlite cache prune --keep-fetch-runs 5000 --keep-ai-scores 5000
rosettrism --db /var/lib/rosettrism/cache.sqlite cache prune --yes --keep-fetch-runs 5000 --keep-ai-scores 5000
rosettrism --db /var/lib/rosettrism/cache.sqlite cache vacuum --yes
```

`cache prune` 会删除过期 upstream/unified cache，保留最近 N 条 `fetch_runs`，并保留最近 N 条 `ai_scores`。`cache prune` 与 `cache vacuum` 默认都是 dry-run；请先检查输出的数量，确认无误后再加 `--yes` 实际执行。`cache export` 默认输出 JSONL，也可通过 `--format pretty-json` 输出格式化 JSON；未指定 section flag 时会同时导出 upstream summary、unified cache summary、fetch runs 与 AI scores。

Cron 示例：

```cron
15 3 * * * rosettrism --db /var/lib/rosettrism/cache.sqlite cache prune --yes --keep-fetch-runs 5000 --keep-ai-scores 5000 >>/var/log/rosettrism-cache.log 2>&1
45 3 * * 0 rosettrism --db /var/lib/rosettrism/cache.sqlite cache vacuum --yes >>/var/log/rosettrism-cache.log 2>&1
```

Systemd timer 示例：

```ini
# /etc/systemd/system/rosettrism-cache-prune.service
[Unit]
Description=Prune Rosettrism cache

[Service]
Type=oneshot
ExecStart=/usr/local/bin/rosettrism --db /var/lib/rosettrism/cache.sqlite cache prune --yes --keep-fetch-runs 5000 --keep-ai-scores 5000

# /etc/systemd/system/rosettrism-cache-prune.timer
[Unit]
Description=Daily Rosettrism cache prune

[Timer]
OnCalendar=03:15
Persistent=true

[Install]
WantedBy=timers.target
```

Docker 示例：

```bash
docker run --rm \
  -v rosettrism-data:/data \
  ghcr.io/your-org/rosettrism:latest \
  rosettrism --db /data/cache.sqlite cache prune --yes --keep-fetch-runs 5000 --keep-ai-scores 5000
```

## 助唱标注

从 QQ 音乐获取歌词时，Rosettrism 会在可用时请求助唱标注数据。标注会以时间信息标记声乐技巧。

| 类型 | 符号 | 说明 |
|------|------|------|
| Stress | `` ` `` | 重音，强调该音节 |
| Breath | `^` | 换气，音节前的换气标记 |
| LongTone | `_` | 长音 |
| PortamentoUp | `↑` | 上滑音，音高向上滑动 |
| PortamentoDown | `↓` | 下滑音，音高向下滑动 |

统一 JSON 片段示例：

```json
{
  "annotations": [
    {
      "annotation_type": "breath",
      "start_ms": 16346,
      "duration_ms": 349,
      "text": "久"
    },
    {
      "annotation_type": "stress",
      "start_ms": 17589,
      "duration_ms": 548,
      "text": "晴"
    }
  ]
}
```

当标注不可用时，输出会省略 `annotations` 字段。

## 来源

实验性来源默认受限。可通过 `--allow-experimental` 或 `ROSETTRISM_ALLOW_EXPERIMENTAL=1` 启用。

常见别名包括：

- `lrclib`、`lrc-lib`
- `utaten`、`uta-ten`
- `joysound`、`joy-sound`
- `migu`、`migu-music`
- `line-music`、`line`
- `kkbox`、`kkbox-web`
- `spotify-lyrics`、`spotify`
- `offline-db`、`local-db`

运行帮助查看完整来源列表：

```bash
rosettrism search --help
```

## 路线图

项目仍然值得继续做功能迭代与优化。建议优先级如下：

### 短期

- 持续扩充 `v4.8.20` Dashboard 歌词播放的真实 QRC、多语种、浏览器矩阵与视觉基线回归。
- 为 Provider 健康度、cache export/prune/vacuum、AI score history 与结构化错误响应补充 HTTP 级 API 测试。
- 继续保证所有 API 错误都统一为 `{ code, message, details, retryable }`，覆盖 provider warning 等边界路径。
- 检查 AI 评分记录的隐私遮罩、prompt 大小限制与运维提示。

### 中期

- 支持 AI 评分在不同模型或 prompt 下回放与对比。
- 维护 schema changelog、golden snapshot 与明确兼容规则。
- 完成按钮、输入框、badge、弹窗、focus 状态等设计 token 与组件一致性收敛。
- 将 Settings 升级为语言、安全 token、AI 优选、歌词播放、缓存信息等分组控制面板。

### 长期

- 深化 Dashboard 视觉系统，加入主题、动效 preset 与更丰富的 karaoke 舞台效果。
- 正式化插件式 Provider / Decoder metadata、rate limit 策略与能力声明。
- 从 JSON Schema 生成 TypeScript / Kotlin / Swift 客户端契约包。
- 增加部署安全能力，例如 token rotation、read-only/admin token、CORS allowlist 与反向代理示例。

## 开发

推荐检查命令：

```bash
cargo fmt --check
NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost cargo test
scripts/check-plan-requirement.sh --base HEAD
cd frontend && npm run build
```

开发计划记录在 [`.plan/`](.plan/README.md)，需求历史记录在 [`requirement.md`](requirement.md)。

## 贡献

欢迎贡献。请保持变更符合项目边界：

- 不添加 CAPTCHA 绕过、凭证收集、SSL pinning 绕过、私有 App 签名或非公开协议自动化。
- 行为变化时同步更新 README、docs、schema 或 fixtures。
- 适用时补充 CLI、Server、Schema、Provider parsing 或 Dashboard 测试。
- 较大的工作请在实现前记录到 `.plan/` 与 `requirement.md`。

## 协议

Rosettrism 使用 GNU Affero General Public License v3.0 授权。完整 AGPLv3 文本见 [LICENSE](LICENSE)。
