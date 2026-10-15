<!-- pane-agent-context:start -->
## Pane

The developer is using Pane for this repository. Pane can manage saved repositories and create user-visible Panes with terminal-backed tools for planning, discussion, implementation, and review work.

This managed guidance was created by [runpane.com](https://runpane.com) for the [Pane repository](https://github.com/dcouple/Pane). Do not delete or overwrite this block; the developer uses it for their workflow and it should remain committed to the main repository unless they explicitly ask to remove it.

Pane mental model: a repository is the saved base repo; a Pane is a user-visible feature/PR workspace (Pane session) that normally maps to one Pane-managed git worktree and branch; a panel/tab is a terminal inside one Pane and shares that Pane's worktree; an agent is the CLI process running in a panel.

Default happy path when the user asks you to use Pane or RunPane: run `runpane doctor --json`; read `runpane agent-context --json`; resolve the saved base repository with `runpane repos list --json` or add it once with `runpane repos add --path <repo> --yes --json`; create one visible Pane (Pane session) for the requested feature/PR with a complete command such as `runpane panes create --repo <repo> --name <name> --agent <agent> --prompt "<task>" --source agent --no-focus --wait-ready --yes --json` or the equivalent `--tool-command <command>` form; then validate with `runpane panels wait` or `runpane panels screen` before reporting progress.

Use Pane when the user wants visible Panes or co-drivable parallel feature/PR workspaces. Do not use Pane as your default private delegation mechanism; for private background decomposition, use your normal subagent/worktree workflow.

Register the main/base repository once. Do not register pre-created git worktrees as separate Pane repositories unless the user explicitly asks.

Use `runpane panes create` for separate visible Panes (Pane sessions) for feature/PR work. Use `runpane panels create` for reviewer/helper tabs inside an existing Pane that should share that Pane's worktree.

Typical workflow: register the saved base repository once; create one Pane (Pane session) per feature/PR; use panels/tabs inside that Pane for helper or reviewer agents that should share the worktree; archive the Pane after the PR is done to remove it from active Panes and clean up its managed worktree when applicable.

Skill routing reference: when the user says `discussion`, `plan`, `simple-plan`, `create-plan`, or `implement`, or asks for the behavior those words imply, treat three references as peer context: Pane's local skill cache under `<PANE_DIR>/skills/`, the Pane Chat orchestrator handoff at `<PANE_DIR>/skills/pane-chat/runpane-orchestrator.md` when present, and the [workflow map](https://github.com/dcouple/skills/raw/main/docs/readme-workflow-map.png).
Use those peer references together to choose the phase: discuss/investigate until the work is clear enough to delegate, then ticket/plan/implement/review/PR-test/teach-back as appropriate. The orchestrator and workflow map may point to different skills; reconcile them with the user's request instead of hardcoding a skill list or treating one reference as subordinate.
For the Pane implementation source of truth for where the skill cache, cached workflow assets, and Pane Chat bootstrap live, reference [PR #291](https://github.com/dcouple/Pane/pull/291): `main/src/services/skillCacheManager.ts` owns `<PANE_DIR>/skills/`, `.sources/dcouple-skills`, and `pane-chat/runpane-orchestrator.md`; `main/src/services/paneChatManager.ts` owns the tiny bootstrap prompt that tells the selected Pane Chat agent to read that guide.
Use GitHub reads against the [Parsa skills folder](https://github.com/dcouple/skills/tree/main/parsa) only to inspect or refresh referenced skill files; do not clone/install the repo unless the user asks.
Do not hardcode a specific assistant brand in workflow guidance. Use the Pane agent or custom tool command the user selected, and use `runpane agents doctor --agent <agent> --repo <selector> --json` only when checking a built-in agent template.

Start with `runpane doctor --json` before taking Pane actions. Use it to understand wrapper/runtime details, daemon reachability, and the next safe commands.

Use `runpane agent-context --json` for full Pane CLI context. Use `runpane agent-context --command "panels wait" --json` or another command name for detailed schema only when needed.

Default to context-safe validation: after creating Panes or sending terminal input, run `runpane panels wait` or `runpane panels screen` before reporting success. Prefer `runpane panels submit` for normal text plus Enter; use `runpane panels input` only for exact bytes such as Ctrl-C or escape sequences.

Common commands:
- `runpane doctor --json`
- `runpane agent-context --json`
- `runpane repos list --json`
- `runpane repos add --path <repo> --yes --json`
- `runpane agents doctor --agent <agent> --repo active --json`
- `runpane panes create --repo active --name <name> --agent <agent> --prompt "<task>" --source agent --no-focus --wait-ready --yes --json`
- `runpane panels create --pane <pane-id> --agent <agent> --source agent --no-focus --wait-ready --yes --json`
- `runpane panels list --pane <pane-id> --json`
- `runpane panels screen --panel <panel-id> --limit 80 --json`
- `runpane panels wait --panel <panel-id> --for ready --timeout-ms 30000 --json`
- `runpane panels submit --panel <panel-id> --text "<answer>" --yes --json`
- `runpane panels input --panel <panel-id> --input-file <path|-> --yes --json`

WSL note: if `runpane doctor --json` cannot find `/tmp/pane-daemon.../daemon.sock` or `runpane` resolves to a broken Windows shim, Pane may be running on Windows. Try `powershell.exe -NoProfile -Command 'Set-Location $env:TEMP; runpane doctor --json'`, then create Panes through the same PowerShell form using the saved WSL repo name or id. Use `runpane agents doctor --agent <agent> --repo <selector> --json` to diagnose the repo environment Pane will actually use.
<!-- pane-agent-context:end -->

# Repository Guidelines

Rosettrism (`rosettrism`, crate name `rosettrism`) is a Rust single-binary lyric toolkit: decode local lyric files, fetch lyrics from 25+ online providers, cache upstream requests in SQLite, aggregate candidates into a stable unified JSON, and serve a React dashboard via an embedded HTTP server. Version 5.0.3, AGPL-3.0-only.

## Project Overview

The binary ships in four modes — CLI (`decode`/`fetch`/`search`/`cache`/`server`), local Axum HTTP API, embedded React dashboard, and a Rust library crate. Decoders handle KRC, QQ QRC/XML, Netease YRC, Apple Music TTML, LRC, plain text, and Rosettrism JSON. Providers cover Kugou, QQ, Netease, Apple Music, Musixmatch, LRCLIB, UtaTen, JOYSOUND, Migu, LINE MUSIC, KKBOX, Genius, Spotify Lyrics, and many more web sources.

## Architecture & Data Flow

```
CLI (clap) ──> ServiceContext ──> aggregate_fetch_inner
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
              cache.get_unified  (miss)     tokio::spawn per source
                    │                           │
                    │              provider_for_with_options(source)
                    │                    │
                    │     ┌────────────┼────────────┐
                    │     ▼            ▼            ▼
                    │  Concrete    ProviderRuntime  CachedProvider
                    │  Provider    (timeout/retry/  (SQLite TTL
                    │              rate-limit)       SHA-256 keys)
                    │     │            │            │
                    │     └────────────┴────────────┘
                    │               ▼
                    │      FetchedLyric {raw, document, annotations}
                    │               │
                    └────> normalize -> aggregate -> AI scoring
                                    │
                                    ▼
                          UnifiedLyric JSON
                            │     │
                    cache.put     fetch_runs + ai_scores
                            │           │
                        Axum JSON    Dashboard
```

**Key architectural patterns:**

- **Trait-based providers**: `LyricProvider` async trait with `search`/`fetch`; a `Source` enum (~27 variants) dispatches to concrete constructors in `provider_for_with_options`. Per-source `ProviderCapabilities` advertises word_timing/translation/romanized/ruby/requires_cookie/experimental.
- **Layered decorator stack (Russian-doll composition)**: concrete provider → `ProviderRuntime` (timeout + retry + token-bucket rate limiter) → `CachedProvider` (SQLite upstream TTL cache). Each layer implements `LyricProvider`, so the service always sees a uniform `Box<dyn LyricProvider>`.
- **Async with tokio**: `tokio::spawn` for concurrent per-source candidate fetching; `tokio::time::timeout` for per-operation deadlines; a `tokio::sync::Mutex` guards the rate limiter; all SQLite calls run through `tokio::task::spawn_blocking` so disk I/O never blocks executor threads.
- **Error handling split**: `thiserror::Error` enum (`src/error.rs`) with `#[from]` conversions for reqwest/rusqlite/io/serde_json defines a library `Result<T>`; `anyhow` + `.context()` is used only at the CLI entry boundary for human-friendly diagnostics.
- **Serde unified model**: `UnifiedLyric`/`LyricDocument` carry `schema_version = "1.0"`, `#[serde(skip_serializing_if = ...)]` for clean empty-field elision, and `sort_and_fill_durations` post-processing so decoders can leave durations unset.
- **Format auto-detection + dispatch**: `decoder/detect.rs` sniffs magic bytes (krc1, QRC QMC magic, TTML, YRC JSON); `decoder/mod.rs` routes to per-format parsers. KRC does XOR+zlib/deflate; QRC does 3DES/DES decryption + XML CDATA/hex/base64 unwrapping.
- **SQLite persistence**: `src/cache/` manages 4 tables (`upstream_cache`, `unified_cache`, `fetch_runs`, `ai_scores`) with schema migrations, foreign keys, SHA-256 cache keys, TTL expiry columns, and an in-process `Arc<std::sync::Mutex<Connection>>` (WAL journal mode + 5s busy_timeout). Every public `UpstreamCache` method is `async fn` and offloads to `spawn_blocking`; the lock guard never crosses an `await`.
- **AI scoring via OpenAI-compatible API**: candidate summaries are hashed and sent to `/chat/completions` with `response_format: json_object`; `best_index` + per-candidate scores persist to `ai_scores` and replayable via `replay_ai_score_for_unified_cache`.
- **Embedded dashboard**: `rust_embed` serves `frontend/dist/` static assets; Axum routes expose `/api/*`; bearer token (`ROSETTRISM_SERVER_TOKEN`) required for non-loopback hosts.

## Key Directories

| Path | Purpose |
|------|---------|
| `src/` | Rust core: lib, model, cli, service, server, cache, cached_provider, error |
| `src/decoder/` | Format decoders: KRC, QRC, YRC, TTML (apple_music), LRC, text, detect, singing_annotations |
| `src/provider/` | 25+ online providers + `runtime.rs` (timeout/retry/rate-limit) + `mod.rs` (Source enum, LyricProvider trait, ProviderManifest) |
| `src/exporter/` | Output formats: LRC, JSON |
| `tests/` | Rust integration tests: `cli.rs` (14 end-to-end), `unified_schema.rs` (schema + fixtures), `decode_fixture.rs` |
| `tests/fixtures/` | Sample lyric files: `sample.qrc`, unified fixtures, ai_score_history |
| `frontend/` | React 19 + Vite 6 + Tailwind 4 + Three.js dashboard |
| `frontend/src/` | `App.jsx`, `LyricPlaybackView.jsx`, `LyricStage3D.jsx`, `views/`, `hooks/`, `api/`, `i18n/`, `styles/`, `utils/` |
| `frontend/tests/` | 7 Playwright E2E specs + fixtures (local WAV for media clock tests) |
| `schema/` | `unified-lyric.schema.json` — JSON Schema 2020-12 for UnifiedLyric |
| `docs/` | `completion-audit-2026-06-06.md`, `provider-plugin-manifest.md`, `unified-json.md` |
| `.plan/` | Dated development plan files (`YYYY-MM-DD-topic.md`), `README.md` naming rules, `TEMPLATE.md` |
| `scripts/` | `check-plan-requirement.sh`, `push-tag.sh`/`.ps1`, `build-release.sh`/`.ps1` |
| `.github/workflows/` | `package.yml` — 3-OS matrix build + Docker container + GitHub Release |

## Development Commands

### Build

```bash
# Rust release binary (target/release/rosettrism[.exe])
cargo build --release

# Frontend dashboard (must run before packaging if frontend changed)
cd frontend && npm install && npm run build

# CI order: frontend first, then Rust (rust-embed embeds frontend/dist/)
cd frontend && npm ci && npm run build && cd .. && cargo build --release --locked
```

### Run

```bash
# Local server + dashboard
cargo run --release -- server --host 127.0.0.1 --port 8080 --open

# CLI decode
cargo run --release -- decode ./lyric.qrc --input-format qrc --format json -o ./lyric.json

# CLI fetch (aggregate multiple sources)
cargo run --release -- fetch "song title artist" --merge-mode tracks --top 1

# Non-local host requires server token
ROSETTRISM_SERVER_TOKEN=secret cargo run --release -- server --host 0.0.0.0 --port 8080
```

### Test

```bash
# Rust tests — NO_PROXY bypass required to prevent proxy interception of localhost
NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost cargo test

# Frontend unit tests (node --test, 8 files, ~39 tests)
cd frontend && npm run test:unit

# Playwright E2E (requires Chromium installed — CDN 403 is a known env limitation)
cd frontend && npm test                          # all 7 specs
cd frontend && npm run test:playback              # core + multilingual
cd frontend && npm run test:playback-benchmark    # visual baselines (4 viewports)
cd frontend && npm run test:media-playback        # phase7 (local WAV)
cd frontend && npm run test:media-session         # phase9 (Media Session)

# Lint
cargo fmt --check        # rustfmt (CI uses dtolnay/rust-toolchain with rustfmt component)
```

### Release

```bash
# Build release with symlink aliases (rstm, rosm)
./scripts/build-release.sh

# Push version tag to remote (reads version from Cargo.toml)
./scripts/push-tag.sh github    # or other remote name

# Plan/requirement consistency check (pre-commit)
./scripts/check-plan-requirement.sh --staged
./scripts/check-plan-requirement.sh --base HEAD~1
```

## Code Conventions & Common Patterns

### Naming

- Rust modules: `snake_case` file names (`cached_provider.rs`, `singing_annotations.rs`)
- Provider files: provider name lowercase (`kugou.rs`, `qq.rs`, `apple_music.rs`, `lrclib.rs`)
- Structs: `PascalCase` (`UnifiedLyric`, `LyricDocument`, `ProviderManifest`)
- Enums: `PascalCase` with `#[serde(rename_all = "snake_case")]` for JSON output (`Source`, `AnnotationType`, `LyricTrackKind`)
- Frontend: `PascalCase` for components (`LyricPlaybackView.jsx`), `camelCase` for hooks (`useThemeSettings.js`)

### Error handling

```rust
// src/error.rs — library error type
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("provider failed: {0}")]
    Provider(String),
    #[error("network request failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("sqlite failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    // ...
}
```

- Library code returns `Result<T>` (thiserror).
- CLI boundary uses `anyhow::Result` with `.context()` for human-friendly messages.
- Never `unwrap()` in library code; CLI entry point may use `anyhow` to convert.

### Provider trait

```rust
// src/provider/mod.rs
#[async_trait]
pub trait LyricProvider: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>>;
    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric>;
}

// Provider construction: concrete -> runtime -> cached decorator
pub fn provider_for_with_options(
    source: Source,
    cookie: Option<String>,
    options: ProviderOptions,
) -> Result<Box<dyn LyricProvider>> { /* match on Source */ }
```

### Provider manifest registry

Each `Source` variant exposes `capabilities()`, `auth_requirement()`, `decoder_output_format()`, and `provider_config()` (timeout_ms/retry/rate_limit). `builtin_provider_registry()` returns `Vec<ProviderManifest>` surfaced via `/api/sources`. Draft manifest filename: `rosettrism-provider.json` (see `docs/provider-plugin-manifest.md`).

### Serde model conventions

- `#[serde(skip_serializing_if = "Option::is_none")]` on optional fields for clean JSON output.
- `#[serde(rename_all = "snake_case")]` on all enums for stable API contract.
- `Default` implementations on all model structs for incremental construction.
- `schema_version = "1.0"` constant in `model.rs`; clients should accept compatible `1.x` optimistically.

### Frontend conventions

- Views are isolated React components in `frontend/src/views/` (`OverviewView`, `FetchView`, `CacheView`, `InspectorView`, `SettingsView`).
- Hooks in `frontend/src/hooks/` manage side effects (`useThemeSettings`, `useAiSettings`, `useCacheEntries`, `useLyricSettings`, `useSidebarState`).
- API client centralized in `frontend/src/api/client.js` — injects `x-rosettrism-token` header, parses JSON errors.
- Token stored in `sessionStorage` (cleared on session end).
- Styles layered: `tokens.css` (design tokens) → `layout.css` → `components.css` → `lyric-stage.css`.
- E2E tests use `page.route` to mock API responses (no external network dependency).

### Requirement & plan workflow

- Every feature-sensitive change (files matching `src/*`, `tests/*`, `schema/*`, `scripts/*`, `frontend/src/*`, `Cargo.toml`, etc.) MUST be accompanied by an update to `requirement.md` or `.plan/*.md`. Enforced by `scripts/check-plan-requirement.sh`.
- `requirement.md`: append a versioned section (`## vX.Y.Z — YYYY-MM-DD — topic`) with goals, acceptance criteria, status checkboxes, and a `.plan/` link.
- `.plan/`: copy `TEMPLATE.md` to `.plan/YYYY-MM-DD-topic.md`; fill background, goals, non-goals, risks, phase checklists, test records.
- Version bumps: minor for features, patch for fixes, major for breaking changes.

## Important Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Binary entry point (90 bytes — delegates to `cli::run()`) |
| `src/lib.rs` | Crate root: module declarations + public re-exports |
| `src/cli.rs` (27KB) | Clap CLI: `decode`, `fetch`, `search`, `server`, `cache` subcommands + `ServiceContext` wiring |
| `src/service.rs` (83KB) | Core orchestration: `aggregate_fetch`, AI scoring, provider fan-out, normalization |
| `src/server/` | Axum routes (`mod.rs`), handlers, search filtering/aggregation, error mapping, `rust_embed` dashboard serving (`assets.rs`) |
| `src/cache/` | SQLite: `mod.rs` (open/migrate/lock), `types.rs`, `upstream.rs`, `unified.rs`, `ai_scores.rs`, `fetch_runs.rs`, `maintenance.rs` (prune/vacuum/export) |
| `src/cached_provider.rs` (11KB) | `CachedProvider` decorator wrapping `LyricProvider` with SQLite TTL |
| `src/model.rs` (6KB) | `UnifiedLyric`, `LyricDocument`, `LyricTrack`, `LyricWord`, `Annotation` types |
| `src/error.rs` | `Error` enum (thiserror) + `Result<T>` alias |
| `src/provider/mod.rs` (20KB) | `Source` enum, `LyricProvider` trait, `ProviderManifest`, `provider_for_with_options` |
| `src/provider/runtime.rs` (11KB) | `ProviderRuntime` decorator: timeout, retry, rate-limit enforcement |
| `src/decoder/mod.rs` | Format dispatch + `decode_bytes` + `InputFormat` |
| `src/decoder/detect.rs` | Magic-byte format auto-detection |
| `Cargo.toml` | Crate metadata, dependencies, `[profile.release]` |
| `frontend/package.json` | Dashboard dependencies, scripts, Playwright test matrix |
| `schema/unified-lyric.schema.json` | JSON Schema 2020-12 for UnifiedLyric validation |
| `.github/workflows/package.yml` | CI: 3-OS matrix build + Docker + GitHub Release |
| `Dockerfile` | Server container image build |

## Runtime/Tooling Preferences

- **Rust**: edition 2021, stable toolchain. CI uses `dtolnay/rust-toolchain@stable` with `rustfmt` component.
- **Node**: version 22 (CI uses `actions/setup-node@v4` with `node-version: 22`). npm, not yarn/pnpm.
- **Frontend bundler**: Vite 6. No Bun usage; standard Node + npm.
- **Package manager**: `npm ci` in CI (respects `package-lock.json`).
- **Release profile**: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`, `debug = false` — optimized for minimal binary size.
- **Database**: SQLite via `rusqlite` with `bundled` feature (no external SQLite dependency).
- **HTTP client**: `reqwest` with `rustls-tls` (no OpenSSL dependency), `brotli`/`gzip`/`deflate` compression.
- **Test proxy bypass**: `NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost` is required for `cargo test` to prevent proxy interception of localhost test traffic.

### Environment variables

| Variable | Purpose |
|----------|---------|
| `ROSETTRISM_SERVER_TOKEN` | Bearer token required when binding server to non-loopback host. Clients send as `x-rosettrism-token` header or `Authorization: Bearer <token>`. |
| `ROSETTRISM_DB` | SQLite cache database path (precedence: `--db` flag > `ROSETTRISM_DB` > `LRC_DECODE_DB` > system data dir) |
| `LRC_DECODE_DB` | Fallback database path env var |
| `NO_PROXY` / `no_proxy` | Set to `127.0.0.1,localhost` for tests to bypass corporate proxies |

## Testing & QA

### Rust tests

- **Unit tests** (`#[cfg(test)] mod tests`): 32 source files have inline test modules. Strong coverage in decoders (QRC has 10 tests including reference-vector and client-payload decryption), providers (wiremock-based `MockServer` tests for apple_music, kugou, lrclib, migu, musixmatch, netease, etc.), cache (TTL, dependency invalidation, fetch_runs, ai_scores with secret redaction), and CLI helpers.
- **Integration tests** (`tests/cli.rs`, 14 tests): decode/fetch/search/server/cache end-to-end via `assert_cmd`.
- **Schema tests** (`tests/unified_schema.rs`): validates `schema/unified-lyric.schema.json` against 4 source fixtures (QQ/QRC, Kugou/KRC, Netease/YRC, LRCLIB/LRC) with back-compat for missing `schema_version`.
- **Server tests** (`src/server/tests.rs`): Axum router integration covering `/api/health`, `/api/stats`, `/api/runs`, `/api/cache/:id`, token auth (401/x-rosettrism-token/Authorization: Bearer), SPA fallback, error mapping.

### Frontend tests

- **Unit tests** (`node --test`, 8 files, ~39 tests): `playbackClock`, `playerSession`, `lyricPlaybackViewModel`, `lyricQuality`, `lyricPlaybackRelease`, `albumColor`, `api/client`, `api/errors`. Run: `cd frontend && npm run test:unit`.
- **E2E specs** (7 Playwright files): `dashboard-smoke` (screenshots), `multilingual-lyrics` (4 languages × 4 viewports, overflow + 44px touch-target assertions), `lyric-playback-core`, `lyric-playback-benchmark` (visual baselines via `toHaveScreenshot` across 4 viewports + 200-line performance sampling), `lyric-playback-release` (real Dragon Knight payload), `media-playback-phase7` (local WAV fixture for media clock), `media-playback-phase9` (Media Session API). All use mocked `page.route` — no external network.

### Known test gaps

- **Playwright Chromium CDN 403**: `npm test` fails in environments without Chromium installed and CDN download blocked (recorded in `requirement.md` v1.8.0). Specs are written and pass with Chromium present.
- **Phase 7 real media**: uses local WAV fixture; real Provider audio URLs remain an external precondition (v2.3.0 Phase 7 incomplete).
- **Phase 9 Firefox runner**: recovery exception unfixed; Edge/Chrome pass 4/4.
- **No isolated tests** for i18n dictionaries, hooks, or views — only indirectly covered via Playwright E2E.
- **No unit tests** for `src/model.rs`, `src/error.rs`, `src/lib.rs`, `src/main.rs`.
- **Screenshot baselines** are platform/browser-specific (win32, firefox-win32 suffixes); may require `--update-snapshots` on other platforms.
