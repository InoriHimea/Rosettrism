# Provider plugin manifest draft

Rosettrism now exposes built-in provider metadata through the same shape planned for a future plugin manifest. The draft manifest filename is `rosettrism-provider.json`.

Dynamic loading of external binaries is intentionally out of scope for this draft. The current implementation uses a built-in plugin registry generated from provider declarations in Rust, so UI and CLI surfaces can consume stable metadata before Rosettrism evaluates WASM or process-based plugins.

## Draft shape

```json
{
  "manifest_version": 1,
  "source": "Netease",
  "name": "netease",
  "source_name": "netease",
  "display_name": "NetEase Cloud Music",
  "capabilities": {
    "search": true,
    "direct_id": true,
    "word_timing": true,
    "translation": true,
    "romanized": true,
    "ruby": false,
    "requires_cookie": false,
    "experimental": false
  },
  "auth": "none",
  "decoder_output_format": ["Yrc", "Lrc"],
  "config": {
    "timeout_ms": 15000,
    "retry": {
      "max_retries": 2,
      "backoff_ms": 500
    },
    "rate_limit": {
      "requests": 30,
      "per_seconds": 60
    }
  }
}
```

## Field notes

- `source_name` is the stable CLI/API source identifier.
- `capabilities` advertises search, direct-id fetch, timing granularity, enrichment tracks, cookie requirements, and experimental status.
- `auth` describes the credential type expected by the provider: `none`, `optional_cookie`, `required_cookie`, `required_token`, or `local_path`.
- `decoder_output_format` declares decoder formats emitted by the provider before normalization.
- `config` centralizes provider timeout, retry, and rate-limit policy so health/statistics endpoints can report policy alongside observed behavior.
