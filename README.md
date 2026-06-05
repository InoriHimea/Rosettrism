# Rosettrism

[中文文档](README-zh.md)

Rosettrism is a Rust single-binary lyric tool. It can decode local KRC/QRC/YRC/LRC/TTML files, fetch lyrics from online sources, cache upstream requests with SQLite, and aggregate multiple sources into a unified JSON result.

Version 4.0 adds a local HTTP server, an embedded dashboard, TTL upstream caching, and multi-source lyric merging. Version 4.2 adds QQ Music singing annotations (助唱标注) support. Current builds make AI candidate selection traceable instead of merely reserved: aggregate responses and cache APIs expose the model, endpoint, candidate hash, scores, reason, and selected source.

## Highlights

- Local decode: KRC, QQ QRC/XML, Netease YRC, Apple Music TTML, LRC, plain text, and Rosettrism JSON.
- Online sources: Kugou, QQ Music, Netease, Apple Music, Musixmatch, PetitLyrics, LRCLIB, UtaTen, JOYSOUND, Migu H5, LINE MUSIC, KKBOX, Genius, AZLyrics, Songtexte, Uta-Net, J-Lyric, J-Total, Kashinavi, UtaMap, Lyrical Nonsense, Animesongz, AWA, TuneCore, RockLyric, Spotify Lyrics, and Offline DB.
- Singing annotations: QQ Music singing annotations (助唱标注) are automatically fetched and included in the output. Annotations mark vocal techniques such as stress (重音), breath (换气), long tone (长音), portamento up (上滑音), and portamento down (下滑音) with per-syllable timing.
- TTL cache: provider `search` and `fetch` calls are cached in SQLite. The default TTL is 7 days. Before TTL expiry, Rosettrism reuses the previous upstream result and does not call the source again.
- Aggregation: when `fetch` is called without `--source`, Rosettrism queries a curated source pool, prefers high-quality timed or word-timed lyrics, and fills missing ruby/reading/romanized tracks when available. Optional OpenAI-compatible AI selection records per-candidate scores and the final reason in `ai_score` and `ai_scores`.
- Unified JSON: default output is multi-track JSON. `--merge-mode inline` can emit a line-oriented merged view.
- Source-specific mode: when `--source` is provided, `--format raw` or `--format json` must also be provided.
- Server mode: `rosettrism server` starts a local Axum API and serves the embedded dashboard. Non-local bindings require `ROSETTRISM_SERVER_TOKEN`; the dashboard can store that token in `sessionStorage` and sends it with API requests.

Rosettrism does not implement CAPTCHA bypass, credential harvesting, SSL pinning bypass, private app signing, or non-public protocol automation.

## Build

```powershell
cargo build --release
```

The binary is created at:

```text
target\release\rosettrism.exe
```

To rebuild the React dashboard:

```powershell
cd frontend
npm install
npm run build
```

The Rust server embeds `frontend/dist`.

## CLI Usage

Decode a local file:

```powershell
rosettrism decode .\lyric.qrc --input-format qrc --format json -o .\lyric.json
rosettrism decode .\lyric.krc --format lrc -o .\lyric.lrc
```

Aggregate sources into unified JSON:

```powershell
rosettrism fetch "song title artist" --merge-mode tracks --top 1
rosettrism fetch "song title artist" --merge-mode inline --top 3 -o .\unified.json
```

Force a refresh and override TTL:

```powershell
rosettrism fetch "song title artist" --ttl 7d --force-refresh
```

Fetch from a specific source:

```powershell
rosettrism fetch "song title artist" --source lrclib --format json
rosettrism fetch "song title artist" --source qq --format raw -o .\qq.raw.txt
```

When `--source` is set, `--format` is required and must be `raw` or `json`.

Search a specific source and save the source raw payload:

```powershell
rosettrism search "song title artist" --source kugou -o .\lyric.krc
```

Search without a source returns aggregate candidates as JSON:

```powershell
rosettrism search "song title artist" -o .\candidates.json
```

## Server API

Start the local server:

```powershell
rosettrism server --host 127.0.0.1 --port 8080 --open
```

If binding to a non-local host, set `ROSETTRISM_SERVER_TOKEN`. Clients must send that value as either `x-rosettrism-token: <token>` or `Authorization: Bearer <token>`; missing or invalid tokens receive a JSON `401` response such as `{ "error": "missing or invalid server token" }`.

Fetch unified JSON:

```powershell
curl -X POST http://127.0.0.1:8080/api/fetch ^
  -H "content-type: application/json" ^
  -H "x-rosettrism-token: %ROSETTRISM_SERVER_TOKEN%" ^
  -d "{\"query\":\"song title artist\",\"merge_mode\":\"tracks\",\"top\":1}"
```

Fetch source raw text:

```powershell
curl -X POST http://127.0.0.1:8080/api/fetch ^
  -H "content-type: application/json" ^
  -H "Authorization: Bearer %ROSETTRISM_SERVER_TOKEN%" ^
  -d "{\"query\":\"song title artist\",\"source\":\"qq\",\"format\":\"raw\"}"
```

Dashboard token usage:

- Localhost servers without `ROSETTRISM_SERVER_TOKEN` need no dashboard token.
- For remote servers, open Settings, enter the same Server Token value configured in `ROSETTRISM_SERVER_TOKEN`, then refresh or retry the API action. The dashboard stores it in `sessionStorage`, so it is cleared when the browser session ends; use **Clear token** to remove it immediately.

Useful endpoints:

- `GET /api/health`
- `GET /api/sources`
- `POST /api/fetch`
- `GET /api/cache`
- `GET /api/cache/:id` (includes `ai_scores` for unified cache records)
- `DELETE /api/cache/:id`
- `POST /api/cache/:id/revalidate`
- `GET /api/stats` (includes cache counts and recent `ai_scores`)

## Cache

The cache database path is chosen in this order:

- `--db <PATH>`
- `ROSETTRISM_DB`
- `LRC_DECODE_DB`
- system data directory fallback

Cache tables include upstream raw operation cache, derived unified cache, fetch runs, traceable AI scoring records, and schema migrations. AI records are linked to `unified_cache` rows and store model, base URL, candidate summary hash, `best_index`, per-candidate heuristic/AI scores, reason, and creation time.

The upstream cache key is based on source, operation, normalized request data, and request version. Cookies and tokens are not included in cache keys.

## Singing Annotations (助唱标注)

When fetching lyrics from QQ Music, Rosettrism automatically retrieves singing annotations if available. These annotations mark vocal techniques on individual syllables with precise timing.

### Annotation Types

| Type | Symbol | Description |
|------|--------|-------------|
| Stress | `` ` `` | 重音 — emphasis on the syllable |
| Breath | `^` | 换气 — breath mark before the syllable |
| LongTone | `_` | 长音 — sustained note |
| PortamentoUp | `↑` | 上滑音 — pitch slides up |
| PortamentoDown | `↓` | 下滑音 — pitch slides down |

### Output Format

Annotations appear in the `annotations` field of the unified JSON output:

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

When no annotations are available (song not supported, or non-QQ source), the `annotations` field is omitted from the output.

### How It Works

1. During QQ Music `fetch`, Rosettrism sends `needSingingAnnotations: true` in the `GetPlayLyricInfo` request.
2. The API returns a hex-encoded encrypted payload in `singingAnnotationsLyric`.
3. Rosettrism decrypts it using the same QRC decryption pipeline, extracts the QRC-format lyric content, and parses annotation symbols embedded before annotated characters.
4. If annotation fetching fails for any reason, the main lyric fetch continues normally with an empty annotations list.

## Sources

Experimental sources are gated by default. Use `--allow-experimental` or `ROSETTRISM_ALLOW_EXPERIMENTAL=1`.

Aliases include:

- `lrclib`, `lrc-lib`
- `utaten`, `uta-ten`
- `joysound`, `joy-sound`
- `migu`, `migu-music`
- `line-music`, `line`
- `kkbox`, `kkbox-web`
- `spotify-lyrics`, `spotify`
- `offline-db`, `local-db`

Run help for the full source list:

```powershell
rosettrism search --help
```

## Verification

```powershell
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --no-fail-fast
```
