# Rosettrism

Rosettrism 是一个 Rust 单二进制歌词工具。它可以解码本地 KRC/QRC/YRC/LRC/TTML 文件，从在线源获取歌词，使用 SQLite 缓存上游请求，并将多个来源聚合为统一的 JSON 结果。

4.0 版本新增了本地 HTTP 服务器、内嵌仪表盘、TTL 上游缓存和多源歌词合并功能。4.2 版本新增了 QQ 音乐助唱标注支持。当前版本已将 AI 候选优选从“预留”升级为可追踪功能：聚合响应和缓存 API 会暴露模型、端点、候选摘要 hash、评分、原因与最终选中来源。

## 功能亮点

- 本地解码：KRC、QQ QRC/XML、网易云 YRC、Apple Music TTML、LRC、纯文本和 Rosettrism JSON。
- 在线源：酷狗、QQ 音乐、网易云、Apple Music、Musixmatch、PetitLyrics、LRCLIB、UtaTen、JOYSOUND、咪咕 H5、LINE MUSIC、KKBOX、Genius、AZLyrics、Songtexte、Uta-Net、J-Lyric、J-Total、Kashinavi、UtaMap、Lyrical Nonsense、Animesongz、AWA、TuneCore、RockLyric、Spotify Lyrics 和离线数据库。
- 助唱标注：自动获取 QQ 音乐的助唱标注数据并包含在输出中。标注标记了重音、换气、长音、上滑音、下滑音等声乐技巧，精确到每个音节的时间。
- TTL 缓存：provider 的 `search` 和 `fetch` 调用结果缓存在 SQLite 中，默认 TTL 为 7 天。TTL 过期前，Rosettrism 复用之前的上游结果，不再重复请求。
- 聚合：当 `fetch` 不指定 `--source` 时，Rosettrism 查询预设的源池，优先选择高质量的逐行/逐字时间轴歌词，并在可用时补充 ruby/reading/romanized 轨道。可选的 OpenAI 兼容 AI 优选会把每个候选的评分和最终原因记录到 `ai_score` / `ai_scores`。
- 统一 JSON：默认输出为多轨 JSON。`--merge-mode inline` 可输出逐行合并视图。
- 指定源模式：指定 `--source` 时，必须同时提供 `--format raw` 或 `--format json`。
- 服务器模式：`rosettrism server` 启动本地 Axum API 并提供内嵌仪表盘。

Rosettrism 不实现验证码绕过、凭证采集、SSL Pinning 绕过、私有应用签名或非公开协议自动化。

## 构建

```powershell
cargo build --release
```

二进制文件生成在：

```text
target\release\rosettrism.exe
```

重新构建 React 仪表盘：

```powershell
cd frontend
npm install
npm run build
```

Rust 服务器内嵌 `frontend/dist`。

## CLI 用法

解码本地文件：

```powershell
rosettrism decode .\lyric.qrc --input-format qrc --format json -o .\lyric.json
rosettrism decode .\lyric.krc --format lrc -o .\lyric.lrc
```

聚合多源为统一 JSON：

```powershell
rosettrism fetch "歌曲名 歌手" --merge-mode tracks --top 1
rosettrism fetch "歌曲名 歌手" --merge-mode inline --top 3 -o .\unified.json
```

强制刷新并覆盖 TTL：

```powershell
rosettrism fetch "歌曲名 歌手" --ttl 7d --force-refresh
```

从指定源获取：

```powershell
rosettrism fetch "歌曲名 歌手" --source lrclib --format json
rosettrism fetch "歌曲名 歌手" --source qq --format raw -o .\qq.raw.txt
```

指定 `--source` 时，`--format` 为必填，值为 `raw` 或 `json`。

搜索指定源并保存原始数据：

```powershell
rosettrism search "歌曲名 歌手" --source kugou -o .\lyric.krc
```

不指定源的搜索返回聚合候选结果 JSON：

```powershell
rosettrism search "歌曲名 歌手" -o .\candidates.json
```

## 服务器 API

启动本地服务器：

```powershell
rosettrism server --host 127.0.0.1 --port 8080 --open
```

绑定非本地地址时需设置 `ROSETTRISM_SERVER_TOKEN`。

获取统一 JSON：

```powershell
curl -X POST http://127.0.0.1:8080/api/fetch ^
  -H "content-type: application/json" ^
  -d "{\"query\":\"歌曲名 歌手\",\"merge_mode\":\"tracks\",\"top\":1}"
```

获取源原始文本：

```powershell
curl -X POST http://127.0.0.1:8080/api/fetch ^
  -H "content-type: application/json" ^
  -d "{\"query\":\"歌曲名 歌手\",\"source\":\"qq\",\"format\":\"raw\"}"
```

可用端点：

- `GET /api/health`
- `GET /api/sources`
- `POST /api/fetch`
- `GET /api/cache`
- `GET /api/cache/:id`（统一缓存记录会包含 `ai_scores`）
- `DELETE /api/cache/:id`
- `POST /api/cache/:id/revalidate`
- `GET /api/stats`（包含缓存计数和最近 `ai_scores`）

## 缓存

缓存数据库路径按以下顺序选择：

- `--db <PATH>`
- `ROSETTRISM_DB`
- `LRC_DECODE_DB`
- 系统数据目录回退

缓存表包括上游原始操作缓存、派生统一缓存、获取记录、可追踪 AI 评分记录和 schema 迁移。AI 记录关联到 `unified_cache` 行，存储模型、base URL、候选摘要 hash、`best_index`、各候选启发式/AI 分数、原因与创建时间。

上游缓存键基于源、操作、规范化请求数据和请求版本。Cookie 和 token 不包含在缓存键中。

## 助唱标注

从 QQ 音乐获取歌词时，Rosettrism 会自动获取助唱标注数据（如果可用）。标注在每个音节上标记声乐技巧，并附带精确的时间信息。

### 标注类型

| 类型 | 符号 | 说明 |
|------|------|------|
| Stress | `` ` `` | 重音 — 强调该音节 |
| Breath | `^` | 换气 — 音节前的换气标记 |
| LongTone | `_` | 长音 — 延长音 |
| PortamentoUp | `↑` | 上滑音 — 音高向上滑动 |
| PortamentoDown | `↓` | 下滑音 — 音高向下滑动 |

### 输出格式

标注出现在统一 JSON 输出的 `annotations` 字段中：

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

当标注不可用时（歌曲不支持或非 QQ 音乐源），输出中省略 `annotations` 字段。

### 工作原理

1. 在 QQ 音乐 `fetch` 过程中，Rosettrism 在 `GetPlayLyricInfo` 请求中发送 `needSingingAnnotations: true`。
2. API 返回 `singingAnnotationsLyric` 字段中的十六进制编码加密数据。
3. Rosettrism 使用与 QRC 相同的解密流程解密数据，提取 QRC 格式的歌词内容，并解析嵌入在标注字符前的标注符号。
4. 如果标注获取因任何原因失败，主歌词获取流程正常继续，标注列表为空。

## 源

实验性源默认被限制。使用 `--allow-experimental` 或 `ROSETTRISM_ALLOW_EXPERIMENTAL=1` 启用。

别名包括：

- `lrclib`、`lrc-lib`
- `utaten`、`uta-ten`
- `joysound`、`joy-sound`
- `migu`、`migu-music`
- `line-music`、`line`
- `kkbox`、`kkbox-web`
- `spotify-lyrics`、`spotify`
- `offline-db`、`local-db`

运行帮助查看完整源列表：

```powershell
rosettrism search --help
```

## 验证

```powershell
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --no-fail-fast
```
