use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::json;

use crate::cache::{error_ttl, CachePut, CachedFetchMetadata, UpstreamCache};
use crate::decoder::InputFormat;
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

pub struct CachedProvider {
    source: Source,
    inner: Box<dyn LyricProvider>,
    cache: UpstreamCache,
    ttl: Duration,
    force_refresh: bool,
}

impl CachedProvider {
    pub fn new(
        source: Source,
        inner: Box<dyn LyricProvider>,
        cache: UpstreamCache,
        ttl: Duration,
        force_refresh: bool,
    ) -> Self {
        Self {
            source,
            inner,
            cache,
            ttl,
            force_refresh,
        }
    }
}

#[async_trait]
impl LyricProvider for CachedProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        #[derive(Serialize)]
        struct SearchKey<'a> {
            query: &'a str,
        }

        let key = UpstreamCache::cache_key(self.source, "search", &SearchKey { query })?;
        if !self.force_refresh {
            if let Some(hit) = self.cache.get_fresh(&key)? {
                return cached_search_hit(hit.body, hit.metadata);
            }
        }

        match self.inner.search(query).await {
            Ok(results) => {
                let body = serde_json::to_vec(&results)?;
                self.cache.put(CachePut {
                    key: &key,
                    source: self.source,
                    operation: "search",
                    status_code: 200,
                    body: &body,
                    metadata: &json!({
                        "payload": "search_results",
                        "query": query,
                        "result_count": results.len(),
                        "first_result": results.first(),
                    }),
                    ttl: self.ttl,
                })?;
                Ok(results)
            }
            Err(err) => {
                let message = err.to_string();
                self.cache.put(CachePut {
                    key: &key,
                    source: self.source,
                    operation: "search",
                    status_code: 599,
                    body: &[],
                    metadata: &json!({ "error": message, "query": query }),
                    ttl: error_ttl(self.ttl),
                })?;
                Err(err)
            }
        }
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let key = UpstreamCache::cache_key(self.source, "fetch", &FetchKey::from(result))?;
        if !self.force_refresh {
            if let Some(hit) = self.cache.get_fresh(&key)? {
                let fetched = cached_fetch_hit(hit.body, hit.metadata)?;
                if cached_fetch_payload_is_usable(&fetched) {
                    return Ok(fetched);
                }
            }
        }

        match self.inner.fetch(result).await {
            Ok(fetched) => {
                let metadata = serde_json::to_value(CachedFetchMetadata {
                    input_format: fetched.input_format,
                    result: Some(result.clone()),
                    document: fetched.document.clone(),
                })?;
                self.cache.put(CachePut {
                    key: &key,
                    source: self.source,
                    operation: "fetch",
                    status_code: 200,
                    body: &fetched.raw,
                    metadata: &metadata,
                    ttl: self.ttl,
                })?;
                Ok(fetched)
            }
            Err(err) => {
                let message = err.to_string();
                self.cache.put(CachePut {
                    key: &key,
                    source: self.source,
                    operation: "fetch",
                    status_code: 599,
                    body: &[],
                    metadata: &json!({ "error": message, "result": result }),
                    ttl: error_ttl(self.ttl),
                })?;
                Err(err)
            }
        }
    }
}

#[derive(Serialize)]
struct FetchKey<'a> {
    id: &'a str,
    title: &'a str,
    artist: &'a str,
    album: &'a Option<String>,
    duration_ms: &'a Option<u32>,
    extra: &'a serde_json::Value,
}

impl<'a> From<&'a SearchResult> for FetchKey<'a> {
    fn from(result: &'a SearchResult) -> Self {
        Self {
            id: &result.id,
            title: &result.title,
            artist: &result.artist,
            album: &result.album,
            duration_ms: &result.duration_ms,
            extra: &result.extra,
        }
    }
}

fn cached_search_hit(body: Vec<u8>, metadata: serde_json::Value) -> Result<Vec<SearchResult>> {
    if let Some(error) = metadata.get("error").and_then(|value| value.as_str()) {
        return Err(Error::Provider(error.to_string()));
    }
    Ok(serde_json::from_slice(&body)?)
}

fn cached_fetch_hit(body: Vec<u8>, metadata: serde_json::Value) -> Result<FetchedLyric> {
    if let Some(error) = metadata.get("error").and_then(|value| value.as_str()) {
        return Err(Error::Provider(error.to_string()));
    }
    let metadata: CachedFetchMetadata = serde_json::from_value(metadata)?;
    Ok(FetchedLyric {
        input_format: metadata.input_format,
        raw: body,
        document: metadata.document,
        annotations: Vec::new(),
    })
}

fn cached_fetch_payload_is_usable(fetched: &FetchedLyric) -> bool {
    fetched.input_format != InputFormat::Krc
        || crate::decoder::krc::decode_raw(&fetched.raw).is_ok()
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use crate::decoder::InputFormat;
    use crate::provider::Source;

    use super::*;

    struct CountingProvider {
        search_count: Arc<AtomicUsize>,
        fetch_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl LyricProvider for CountingProvider {
        async fn search(&self, _query: &str) -> Result<Vec<SearchResult>> {
            self.search_count.fetch_add(1, Ordering::SeqCst);
            Ok(vec![SearchResult {
                source: Source::Lrclib,
                id: "1".into(),
                title: "Song".into(),
                artist: "Artist".into(),
                album: None,
                duration_ms: Some(1_000),
                extra: json!({ "id": 1 }),
            }])
        }

        async fn fetch(&self, _result: &SearchResult) -> Result<FetchedLyric> {
            self.fetch_count.fetch_add(1, Ordering::SeqCst);
            Ok(FetchedLyric {
                input_format: InputFormat::Lrc,
                raw: b"[00:01.00]Hi\n".to_vec(),
                document: None,
                annotations: Vec::new(),
            })
        }
    }

    #[tokio::test]
    async fn caches_provider_search_and_fetch_until_ttl_expires() {
        let path = std::env::temp_dir().join(format!(
            "rosettrism-cache-provider-{}.sqlite",
            crate::cache::now_unix()
        ));
        let cache = UpstreamCache::open(&path).unwrap();
        let cache_for_assert = cache.clone();
        let search_count = Arc::new(AtomicUsize::new(0));
        let fetch_count = Arc::new(AtomicUsize::new(0));
        let provider = CachedProvider::new(
            Source::Lrclib,
            Box::new(CountingProvider {
                search_count: search_count.clone(),
                fetch_count: fetch_count.clone(),
            }),
            cache,
            Duration::from_secs(60),
            false,
        );

        let first = provider.search("Song Artist").await.unwrap();
        let second = provider.search("Song Artist").await.unwrap();
        assert_eq!(first[0].id, second[0].id);
        assert_eq!(search_count.load(Ordering::SeqCst), 1);

        let _ = provider.fetch(&first[0]).await.unwrap();
        let _ = provider.fetch(&first[0]).await.unwrap();
        assert_eq!(fetch_count.load(Ordering::SeqCst), 1);

        let entries = cache_for_assert.list(10).unwrap();
        assert!(entries
            .iter()
            .any(|entry| entry.query.as_deref() == Some("Song Artist")));
        assert!(entries
            .iter()
            .any(|entry| entry.title.as_deref() == Some("Song")
                && entry.artist.as_deref() == Some("Artist")
                && entry.item_id.as_deref() == Some("1")));

        drop(provider);
        drop(cache_for_assert);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn invalid_cached_krc_payload_is_refreshed() {
        let path = std::env::temp_dir().join(format!(
            "rosettrism-cache-provider-invalid-krc-{}.sqlite",
            crate::cache::now_unix()
        ));
        let cache = UpstreamCache::open(&path).unwrap();
        let result = SearchResult {
            source: Source::Kugou,
            id: "kid".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: Some(1_000),
            extra: json!({
                "id": "kid",
                "accesskey": "akey"
            }),
        };
        let key =
            UpstreamCache::cache_key(Source::Kugou, "fetch", &FetchKey::from(&result)).unwrap();
        let metadata = serde_json::to_value(CachedFetchMetadata {
            input_format: InputFormat::Krc,
            result: Some(result.clone()),
            document: None,
        })
        .unwrap();
        cache
            .put(CachePut {
                key: &key,
                source: Source::Kugou,
                operation: "fetch",
                status_code: 200,
                body: b"krc1not-deflate",
                metadata: &metadata,
                ttl: Duration::from_secs(60),
            })
            .unwrap();

        let fetch_count = Arc::new(AtomicUsize::new(0));
        let provider = CachedProvider::new(
            Source::Kugou,
            Box::new(CountingProvider {
                search_count: Arc::new(AtomicUsize::new(0)),
                fetch_count: fetch_count.clone(),
            }),
            cache.clone(),
            Duration::from_secs(60),
            false,
        );

        let fetched = provider.fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "[00:01.00]Hi\n");

        drop(provider);
        drop(cache);
        let _ = std::fs::remove_file(path);
    }
}
