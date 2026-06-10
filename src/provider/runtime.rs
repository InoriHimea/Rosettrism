use std::future::Future;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::provider::{FetchedLyric, LyricProvider, ProviderConfig, SearchResult, Source};
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRequestPolicy {
    pub timeout: Duration,
    pub retry: ProviderRetryPolicy,
    pub rate_limit: ProviderRateLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRetryPolicy {
    pub max_retries: u8,
    pub backoff: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRateLimit {
    pub requests: u32,
    pub per_seconds: u64,
}

impl From<ProviderConfig> for ProviderRequestPolicy {
    fn from(config: ProviderConfig) -> Self {
        Self {
            timeout: Duration::from_millis(config.timeout_ms),
            retry: ProviderRetryPolicy {
                max_retries: config.retry.max_retries,
                backoff: Duration::from_millis(config.retry.backoff_ms),
            },
            rate_limit: ProviderRateLimit {
                requests: config.rate_limit.requests,
                per_seconds: config.rate_limit.per_seconds,
            },
        }
    }
}

pub struct ProviderRuntime {
    source: Source,
    inner: Box<dyn LyricProvider>,
    policy: ProviderRequestPolicy,
    rate_limiter: Option<RateLimiter>,
}

impl ProviderRuntime {
    pub fn new(
        source: Source,
        inner: Box<dyn LyricProvider>,
        policy: ProviderRequestPolicy,
    ) -> Self {
        let rate_limiter = RateLimiter::new(policy.rate_limit);
        Self {
            source,
            inner,
            policy,
            rate_limiter,
        }
    }

    async fn acquire_rate_limit(&self) {
        if let Some(rate_limiter) = &self.rate_limiter {
            rate_limiter.acquire().await;
        }
    }

    async fn run_with_policy<T, Fut, F>(&self, operation: &str, mut run: F) -> Result<T>
    where
        T: Send,
        F: FnMut() -> Fut + Send,
        Fut: Future<Output = Result<T>> + Send,
    {
        let attempts = u32::from(self.policy.retry.max_retries) + 1;
        for attempt in 1..=attempts {
            self.acquire_rate_limit().await;
            match tokio::time::timeout(self.policy.timeout, run()).await {
                Ok(Ok(value)) => return Ok(value),
                Ok(Err(err)) if attempt < attempts && is_retryable(&err) => {
                    tokio::time::sleep(self.policy.retry.backoff).await;
                }
                Ok(Err(err)) => return Err(err),
                Err(_) if attempt < attempts => {
                    tokio::time::sleep(self.policy.retry.backoff).await;
                }
                Err(_) => {
                    return Err(Error::Provider(format!(
                        "{} {} timed out after {}ms",
                        self.source.cli_name(),
                        operation,
                        self.policy.timeout.as_millis()
                    )));
                }
            }
        }

        Err(Error::Provider(format!(
            "{} {} failed after {} attempts",
            self.source.cli_name(),
            operation,
            attempts
        )))
    }
}

#[async_trait]
impl LyricProvider for ProviderRuntime {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.run_with_policy("search", || self.inner.search(query))
            .await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        self.run_with_policy("fetch", || self.inner.fetch(result))
            .await
    }
}

#[derive(Debug)]
struct RateLimiter {
    state: Mutex<RateLimitState>,
}

#[derive(Debug)]
struct RateLimitState {
    tokens: f64,
    last_refill: Instant,
    capacity: f64,
    refill_per_second: f64,
}

impl RateLimiter {
    fn new(rate_limit: ProviderRateLimit) -> Option<Self> {
        if rate_limit.requests == 0 || rate_limit.per_seconds == 0 {
            return None;
        }

        let capacity = f64::from(rate_limit.requests);
        let refill_per_second = f64::from(rate_limit.requests) / rate_limit.per_seconds as f64;
        Some(Self {
            state: Mutex::new(RateLimitState {
                tokens: capacity,
                last_refill: Instant::now(),
                capacity,
                refill_per_second,
            }),
        })
    }

    async fn acquire(&self) {
        loop {
            let wait = {
                let mut state = self.state.lock().await;
                state.refill();
                if state.tokens >= 1.0 {
                    state.tokens -= 1.0;
                    return;
                }

                let missing = 1.0 - state.tokens;
                Duration::from_secs_f64(missing / state.refill_per_second)
            };

            tokio::time::sleep(wait).await;
        }
    }
}

impl RateLimitState {
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        if elapsed > 0.0 {
            self.tokens = (self.tokens + elapsed * self.refill_per_second).min(self.capacity);
            self.last_refill = now;
        }
    }
}

fn is_retryable(error: &Error) -> bool {
    match error {
        Error::Network(err) => err.is_timeout() || err.is_connect(),
        Error::Provider(message) => {
            let message = message.to_ascii_lowercase();
            message.contains("timed out")
                || message.contains("timeout")
                || message.contains("transient")
                || message.contains("temporarily")
                || message.contains("temporary")
                || message.contains("rate limit")
                || message.contains("too many requests")
                || message.contains("429")
                || message.contains("500")
                || message.contains("502")
                || message.contains("503")
                || message.contains("504")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use async_trait::async_trait;
    use serde_json::json;

    use crate::decoder::InputFormat;
    use crate::model::LyricDocument;
    use crate::provider::{
        FetchedLyric, LyricProvider, ProviderConfig, ProviderRateLimit, ProviderRetryPolicy,
        SearchResult, Source,
    };
    use crate::{Error, Result};

    use super::{ProviderRequestPolicy, ProviderRuntime};

    struct FlakyProvider {
        search_attempts: Arc<AtomicUsize>,
        fetch_attempts: Arc<AtomicUsize>,
        fail_searches: usize,
        fail_fetches: usize,
    }

    #[async_trait]
    impl LyricProvider for FlakyProvider {
        async fn search(&self, _query: &str) -> Result<Vec<SearchResult>> {
            let attempt = self.search_attempts.fetch_add(1, Ordering::SeqCst);
            if attempt < self.fail_searches {
                return Err(Error::Provider(format!(
                    "transient search failure {attempt}"
                )));
            }

            Ok(vec![SearchResult {
                source: Source::Lrclib,
                id: "1".into(),
                title: "Song".into(),
                artist: "Artist".into(),
                album: None,
                duration_ms: Some(1_000),
                extra: json!({"kind": "test"}),
            }])
        }

        async fn fetch(&self, _result: &SearchResult) -> Result<FetchedLyric> {
            let attempt = self.fetch_attempts.fetch_add(1, Ordering::SeqCst);
            if attempt < self.fail_fetches {
                return Err(Error::Provider(format!(
                    "transient fetch failure {attempt}"
                )));
            }

            Ok(FetchedLyric {
                input_format: InputFormat::Lrc,
                raw: b"[00:00.00]ok\n".to_vec(),
                document: Some(LyricDocument::default()),
                annotations: Vec::new(),
            })
        }
    }

    fn test_policy(max_retries: u8) -> ProviderRequestPolicy {
        ProviderRequestPolicy::from(ProviderConfig {
            timeout_ms: 1_000,
            retry: ProviderRetryPolicy {
                max_retries,
                backoff_ms: 1,
            },
            rate_limit: ProviderRateLimit {
                requests: 1_000,
                per_seconds: 1,
            },
        })
    }

    #[tokio::test]
    async fn retries_search_until_success() {
        let search_attempts = Arc::new(AtomicUsize::new(0));
        let fetch_attempts = Arc::new(AtomicUsize::new(0));
        let runtime = ProviderRuntime::new(
            Source::Lrclib,
            Box::new(FlakyProvider {
                search_attempts: search_attempts.clone(),
                fetch_attempts,
                fail_searches: 2,
                fail_fetches: 0,
            }),
            test_policy(2),
        );

        let results = runtime.search("query").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(search_attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn stops_retrying_after_policy_limit() {
        let search_attempts = Arc::new(AtomicUsize::new(0));
        let fetch_attempts = Arc::new(AtomicUsize::new(0));
        let runtime = ProviderRuntime::new(
            Source::Lrclib,
            Box::new(FlakyProvider {
                search_attempts: search_attempts.clone(),
                fetch_attempts,
                fail_searches: 3,
                fail_fetches: 0,
            }),
            test_policy(1),
        );

        let err = runtime.search("query").await.unwrap_err().to_string();
        assert!(
            err.contains("failed after 2 attempts") || err.contains("transient search failure")
        );
        assert_eq!(search_attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retries_fetch_after_search_success() {
        let search_attempts = Arc::new(AtomicUsize::new(0));
        let fetch_attempts = Arc::new(AtomicUsize::new(0));
        let runtime = ProviderRuntime::new(
            Source::Lrclib,
            Box::new(FlakyProvider {
                search_attempts,
                fetch_attempts: fetch_attempts.clone(),
                fail_searches: 0,
                fail_fetches: 1,
            }),
            test_policy(1),
        );

        let result = runtime.search("query").await.unwrap().remove(0);
        let fetched = runtime.fetch(&result).await.unwrap();
        assert_eq!(fetched.raw, b"[00:00.00]ok\n".to_vec());
        assert_eq!(fetch_attempts.load(Ordering::SeqCst), 2);
    }
}
