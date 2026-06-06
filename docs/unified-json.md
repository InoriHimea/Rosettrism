# Unified JSON schema and compatibility

Rosettrism aggregate fetches return a `UnifiedLyric` JSON object. The canonical schema is `schema/unified-lyric.schema.json` and the current `schema_version` is `1.0`.

## Top-level contract

- `schema_version` identifies the payload contract. Clients should parse known `1.x` fields optimistically and fall back to a conservative display for newer major versions.
- `mode` is either `tracks` or `inline`.
- `meta` contains best-effort song metadata merged from the selected base candidate.
- `source_refs` lists providers that contributed selected tracks; `cache_refs` identifies fetched provider/result/format inputs.
- `score` is an aggregate quality summary for ranking and diagnostics, not a validation requirement for display.
- `warnings` is advisory. Clients should not fail rendering because warnings are present.
- Unknown fields are forward-compatible and should be ignored by clients.

## `tracks` mode

`tracks` mode preserves each selected source/enrichment as a separate `LyricTrack`:

- The `original` track is the base lyric chosen for timing and text.
- Optional `translation`, `ruby`, `reading`, and `romanized` tracks may be present when a compatible source is available.
- Each track has its own `document.lines`, `quality`, `source`, and optional BCP-47-ish `language` hint.
- Track line counts do not have to match. Clients should align tracks by nearest `start_ms` when building their own merged view.
- A `plain_fallback` track kind is reserved for untimed text fallback output.

Use `tracks` mode when clients want provenance, source-specific quality, or custom alignment behavior.

## `inline` mode

`inline` mode keeps the base lyric line text in `inline_lines` and attaches compatible enrichment fields to the nearest base line:

- `inline_lines[].text`, `start_ms`, and `duration_ms` come from the base `original` lyric.
- `translation`, `reading`, `romanized`, and `ruby` are copied from the base line when already present, or filled from selected tracks when a nearby line exists.
- Current alignment treats lines within 1,500 ms as compatible.
- `source_refs` on each inline line names providers that may have contributed the base text or enrichment.
- `tracks` can still be included in an inline response, so clients can inspect provenance or rebuild a different merge.

Use `inline` mode when clients want a simple line-oriented rendering model.

## Annotations

`annotations` are top-level timed singing marks, currently sourced from QQ Music when available. Each annotation contains:

- `annotation_type`: one of `stress`, `breath`, `long_tone`, `portamento_up`, or `portamento_down`.
- `start_ms` and `duration_ms`: absolute timing in milliseconds.
- `text`: the lyric syllable or marker text associated with the vocal technique.

Annotations are independent from tracks and inline lines. Clients should overlay them by time and ignore unsupported annotation types in future schema versions.

## Ruby, translation, reading, and romanized fields

Compatibility rules:

- `ruby` is an array of spans with `start_char`, `end_char`, `text`, and `reading`. Character offsets are UTF-8 string character indexes, not byte offsets.
- `translation` is a human-language translation for the original line. It may appear on `LyricLine` or `InlineLyricLine`.
- `reading` is a pronunciation/reading representation, commonly kana for Japanese lyrics.
- `romanized` is a Latin-script representation and may coexist with `reading`.
- These enrichment fields are optional. Missing fields mean “not available”, not validation failure.
- Empty arrays such as `ruby: []` are equivalent to omitted `ruby` in serialized output.
- Clients should prefer exact line-level enrichment when present and otherwise align by time with a tolerance chosen for the product.

## Client parsing strategy

Recommended clients should:

1. Read `schema_version` first.
2. Accept `1.x` payloads while ignoring unknown fields.
3. If a newer major version is encountered, fall back to displaying `tracks[0].document.lines` or `inline_lines` when those fields are present.
4. Treat optional enrichment and annotations as additive; do not block base lyric display if they are missing or malformed.
5. Preserve provider/source labels when exposing diagnostics or user source selection.
