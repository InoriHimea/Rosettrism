# Rosettrism

[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![Frontend](https://img.shields.io/badge/Dashboard-React%20%2B%20Vite-61dafb.svg)](frontend/package.json)
[![Schema](https://img.shields.io/badge/Unified%20JSON-1.0-blue.svg)](schema/unified-lyric.schema.json)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPLv3-blue.svg)](LICENSE)

[中文文档](README-zh.md) · [Unified JSON guide](docs/unified-json.md) · [Completion audit](docs/completion-audit-2026-06-06.md)

Rosettrism is a Rust single-binary lyric toolkit for decoding local lyric files, fetching lyrics from online providers, caching upstream requests, and aggregating multiple candidates into a stable unified JSON response. It ships as a CLI, a local Axum HTTP API, and an embedded React dashboard.

## Project status

The previous planning round has been completed and audited. Rosettrism is usable as a CLI/server/dashboard today, but the project should continue with focused feature iteration and optimization rather than being considered finished. The next most valuable work is reliability and maintainability: API end-to-end tests, unified API error codes, provider health metrics, cache maintenance tooling, AI scoring replay, schema compatibility governance, and dashboard UX polish.

See [docs/completion-audit-2026-06-06.md](docs/completion-audit-2026-06-06.md) for the detailed completion matrix and recommended next steps.

## Features

- **Local decoding**: KRC, QQ QRC/XML, Netease YRC, Apple Music TTML, LRC, plain text, and Rosettrism JSON.
- **Many online sources**: Kugou, QQ Music, Netease, Apple Music, Musixmatch, PetitLyrics, LRCLIB, UtaTen, JOYSOUND, Migu H5, LINE MUSIC, KKBOX, Genius, AZLyrics, Songtexte, Uta-Net, J-Lyric, J-Total, Kashinavi, UtaMap, Lyrical Nonsense, Animesongz, AWA, TuneCore, RockLyric, Spotify Lyrics, and Offline DB.
- **Unified lyric model**: Multi-track JSON by default, optional inline merged lines, `schema_version`, annotations, translations, readings, ruby, and romanized tracks.
- **Singing annotations**: QQ Music singing annotations (助唱标注) are fetched when available and mapped to timed per-syllable vocal-technique markers.
- **TTL cache**: Provider `search` and `fetch` calls are cached in SQLite with a default TTL of 7 days.
- **Aggregation and AI traceability**: Aggregated fetches prefer high-quality timed or word-timed lyrics; optional OpenAI-compatible AI selection records model, endpoint, candidate hash, scores, reason, and selected source.
- **Observability**: Fetch runs record recent queries, sources, modes, statuses, messages, cache hits/stores, provider warnings, AI skips, and no-lyrics outcomes.
- **Local dashboard**: `rosettrism server` serves an embedded dashboard and HTTP API. Non-local bindings require `ROSETTRISM_SERVER_TOKEN`.

Rosettrism does **not** implement CAPTCHA bypass, credential harvesting, SSL pinning bypass, private app signing, or non-public protocol automation.

## Table of contents

- [Installation](#installation)
- [Quick start](#quick-start)
- [CLI usage](#cli-usage)
- [Server API](#server-api)
- [Unified JSON](#unified-json)
- [Cache and observability](#cache-and-observability)
- [Singing annotations](#singing-annotations)
- [Sources](#sources)
- [Roadmap](#roadmap)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## Installation

### Build the Rust binary

```bash
cargo build --release
```

The release binary is created at:

```text
target/release/rosettrism
```

On Windows, the binary is:

```text
target\release\rosettrism.exe
```

### Build the embedded dashboard

```bash
cd frontend
npm install
npm run build
```

The Rust server embeds `frontend/dist`. Rebuild the dashboard before packaging if frontend files changed.

## Quick start

Decode a local lyric file:

```bash
rosettrism decode ./lyric.qrc --input-format qrc --format json -o ./lyric.json
```

Aggregate multiple sources into unified JSON:

```bash
rosettrism fetch "song title artist" --merge-mode tracks --top 1 -o ./unified.json
```

Start the local server and dashboard:

```bash
rosettrism server --host 127.0.0.1 --port 8080 --open
```

## CLI usage

### Decode local files

```bash
rosettrism decode ./lyric.qrc --input-format qrc --format json -o ./lyric.json
rosettrism decode ./lyric.krc --format lrc -o ./lyric.lrc
```

### Aggregate sources

```bash
rosettrism fetch "song title artist" --merge-mode tracks --top 1
rosettrism fetch "song title artist" --merge-mode inline --top 3 -o ./unified.json
```

Force a refresh and override TTL:

```bash
rosettrism fetch "song title artist" --ttl 7d --force-refresh
```

### Fetch from a specific source

When `--source` is set for source-specific fetching, use `--format raw` or `--format json`.

```bash
rosettrism fetch "song title artist" --source lrclib --format json
rosettrism fetch "song title artist" --source qq --format raw -o ./qq.raw.txt
```

### Search candidates

Search a specific source and save the selected raw payload:

```bash
rosettrism search "song title artist" --source kugou -o ./lyric.krc
```

Search without a source to return aggregated candidates as JSON:

```bash
rosettrism search "song title artist" -o ./candidates.json
```

## Server API

Start the local server:

```bash
rosettrism server --host 127.0.0.1 --port 8080 --open
```

If binding to a non-local host, set `ROSETTRISM_SERVER_TOKEN`. Clients must send that value as either `x-rosettrism-token: <token>` or `Authorization: Bearer <token>`. Missing or invalid tokens receive a JSON `401` response such as `{ "error": "missing or invalid server token" }`.

Fetch unified JSON:

```bash
curl -X POST http://127.0.0.1:8080/api/fetch \
  -H "content-type: application/json" \
  -H "x-rosettrism-token: ${ROSETTRISM_SERVER_TOKEN}" \
  -d '{"query":"song title artist","merge_mode":"tracks","top":1}'
```

Fetch source raw text:

```bash
curl -X POST http://127.0.0.1:8080/api/fetch \
  -H "content-type: application/json" \
  -H "Authorization: Bearer ${ROSETTRISM_SERVER_TOKEN}" \
  -d '{"query":"song title artist","source":"qq","format":"raw"}'
```

Dashboard token behavior:

- Localhost servers without `ROSETTRISM_SERVER_TOKEN` do not require a dashboard token.
- Remote servers require the same token in dashboard Settings. The dashboard stores it in `sessionStorage` and clears it when the browser session ends; use **Clear Token** to remove it immediately.

Available endpoints:

- `GET /api/health`
- `GET /api/sources`
- `POST /api/fetch`
- `GET /api/cache`
- `GET /api/cache/:id`
- `DELETE /api/cache/:id`
- `POST /api/cache/:id/revalidate`
- `GET /api/runs`
- `GET /api/stats`

## Unified JSON

Unified aggregate output is described by [`schema/unified-lyric.schema.json`](schema/unified-lyric.schema.json). Compatibility rules for tracks, inline lines, annotations, ruby, translations, readings, and romanization are documented in [`docs/unified-json.md`](docs/unified-json.md).

Client parsers should ignore unknown fields so newer Rosettrism builds can add optional data without breaking existing apps. Use `schema_version` for downgrade behavior: accept compatible `1.x` payloads optimistically, and for newer major versions fall back to `tracks[0].document.lines` or `inline_lines` when present.

## Cache and observability

Cache database path precedence:

1. `--db <PATH>`
2. `ROSETTRISM_DB`
3. `LRC_DECODE_DB`
4. System data directory fallback

The cache stores upstream raw operations, derived unified responses, fetch-run records, traceable AI score records, and schema migrations. Upstream cache keys are based on source, operation, normalized request data, and request version. Cookies and tokens are not included in cache keys.

Fetch-run observability covers aggregate fetches, multi-source searches, selected result fetches, and aggregate member fetches. The dashboard Overview/Cache views and `/api/runs` expose statuses such as `provider_warning`, `ai_skipped`, `no_lyrics_found`, `cache_hit`, and `cache_store`.

## Singing annotations

When fetching QQ Music lyrics, Rosettrism requests singing annotation data when available. These annotations mark vocal techniques with timing information.

| Type | Symbol | Meaning |
|------|--------|---------|
| Stress | `` ` `` | Emphasized syllable |
| Breath | `^` | Breath marker before a syllable |
| LongTone | `_` | Extended tone |
| PortamentoUp | `↑` | Pitch slides upward |
| PortamentoDown | `↓` | Pitch slides downward |

Example unified JSON fragment:

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

If annotations are unavailable, the output omits the `annotations` field.

## Sources

Experimental sources are restricted by default. Enable them with `--allow-experimental` or `ROSETTRISM_ALLOW_EXPERIMENTAL=1`.

Common aliases include:

- `lrclib`, `lrc-lib`
- `utaten`, `uta-ten`
- `joysound`, `joy-sound`
- `migu`, `migu-music`
- `line-music`, `line`
- `kkbox`, `kkbox-web`
- `spotify-lyrics`, `spotify`
- `offline-db`, `local-db`

Run help for the full source list:

```bash
rosettrism search --help
```

## Roadmap

The project still benefits from feature iteration and optimization. Recommended priorities:

### Short term

- Add server/API end-to-end tests for token handling, `/api/cache/:id`, `/api/runs`, and AI score output.
- Standardize API errors as `{ code, message, details, retryable }` so the dashboard can show better recovery actions.
- Add a lightweight plan/requirement consistency check for development workflow hygiene.
- Review AI scoring records for privacy, masking, and payload-size limits.

### Medium term

- Add cache maintenance commands for prune, export, vacuum, and migration status.
- Build provider health metrics from `fetch_runs`, including success rate, warning rate, latency, and recent failures.
- Support AI scoring replay/comparison across model or prompt changes.
- Maintain schema changelog, golden snapshots, and explicit compatibility rules.

### Long term

- Deepen the dashboard visual system with themes, motion presets, and richer karaoke-stage effects.
- Explore plugin-style provider/decoder metadata, rate limits, and capability declarations.
- Generate client contract packages from JSON Schema for TypeScript, Kotlin, or Swift.
- Add deployment security features such as token rotation, read-only/admin tokens, CORS allowlists, and reverse-proxy examples.

## Development

Recommended checks:

```bash
cargo fmt --check
NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost cargo test
cd frontend && npm run build
```

Development planning is tracked under [`.plan/`](.plan/README.md). Requirement history is tracked in [`requirement.md`](requirement.md).

## Contributing

Contributions are welcome. Please keep changes aligned with the project scope:

- Do not add CAPTCHA bypass, credential harvesting, SSL pinning bypass, private app signing, or non-public protocol automation.
- Update README/docs/schema/fixtures when behavior changes.
- Add or update tests for CLI, server, schema, provider parsing, or dashboard behavior when applicable.
- Record larger work in `.plan/` and `requirement.md` before implementation.

## License

Rosettrism is licensed under the GNU Affero General Public License v3.0. See [LICENSE](LICENSE) for the full AGPLv3 text.
