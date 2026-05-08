use async_trait::async_trait;

use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

pub struct UnsupportedExperimentalProvider {
    source: Source,
    name: &'static str,
    detail: &'static str,
}

impl UnsupportedExperimentalProvider {
    pub fn new(source: Source, name: &'static str, detail: &'static str) -> Self {
        Self {
            source,
            name,
            detail,
        }
    }

    fn error(&self) -> Error {
        Error::Provider(format!(
            "{} ({}) is listed as an experimental source, but it is not implemented yet: {}",
            self.name,
            self.source.cli_name(),
            self.detail
        ))
    }
}

#[async_trait]
impl LyricProvider for UnsupportedExperimentalProvider {
    async fn search(&self, _query: &str) -> Result<Vec<SearchResult>> {
        Err(self.error())
    }

    async fn fetch(&self, _result: &SearchResult) -> Result<FetchedLyric> {
        Err(self.error())
    }
}
