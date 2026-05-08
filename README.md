# Rosettrism

Rosettrism is a Rust single-binary lyric tool. It can decode local KRC/QRC/YRC/LRC/TTML files, fetch lyrics from online sources, cache upstream requests with SQLite, and aggregate multiple sources into a unified JSON result.

Version 4.0 adds a local HTTP server, an embedded dashboard, TTL upstream caching, and multi-source lyric merging.

## Highlights

- Local decode: KRC, QQ QRC/XML, Netease YRC, Apple Music TTML, LRC, plain text, and Rosettrism JSON.
- Online sources: Kugou, QQ Music, Netease, Apple Music, Musixmatch, PetitLyrics, LRCLIB, UtaTen, JOYSOUND, Migu H5, LINE MUSIC, KKBOX, Genius, AZLyrics, Songtexte, Uta-Net, J-Lyric, J-Total, Kashinavi, UtaMap, Lyrical Nonsense, Animesongz, AWA, TuneCore, RockLyric, Spotify Lyrics, and Offline DB.
- TTL cache: provider `search` and `fetch` calls are cached in SQLite. The default TTL is 7 days. Before TTL expiry, Rosettrism reuses the previous upstream result and does not call the source again.
- Aggregation: when `fetch` is called without `--source`, Rosettrism queries a curated source pool, prefers high-quality timed or word-timed lyrics, and fills missing ruby/reading/romanized tracks when available.
- Unified JSON: default output is multi-track JSON. `--merge-mode inline` can emit a line-oriented merged view.
- Source-specific mode: when `--source` is provided, `--format raw` or `--format json` must also be provided.
- Server mode: `rosettrism server` starts a local Axum API and serves the embedded dashboard.

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

If binding to a non-local host, set `ROSETTRISM_SERVER_TOKEN`.

Fetch unified JSON:

```powershell
curl -X POST http://127.0.0.1:8080/api/fetch ^
  -H "content-type: application/json" ^
  -d "{\"query\":\"song title artist\",\"merge_mode\":\"tracks\",\"top\":1}"
```

Fetch source raw text:

```powershell
curl -X POST http://127.0.0.1:8080/api/fetch ^
  -H "content-type: application/json" ^
  -d "{\"query\":\"song title artist\",\"source\":\"qq\",\"format\":\"raw\"}"
```

Useful endpoints:

- `GET /api/health`
- `GET /api/sources`
- `POST /api/fetch`
- `GET /api/cache`
- `GET /api/cache/:id`
- `DELETE /api/cache/:id`
- `POST /api/cache/:id/revalidate`
- `GET /api/stats`

## Cache

The cache database path is chosen in this order:

- `--db <PATH>`
- `ROSETTRISM_DB`
- `LRC_DECODE_DB`
- system data directory fallback

Cache tables include upstream raw operation cache, derived unified cache, fetch runs, AI score placeholders, and schema migrations.

The upstream cache key is based on source, operation, normalized request data, and request version. Cookies and tokens are not included in cache keys.

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
