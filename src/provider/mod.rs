use async_trait::async_trait;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::decoder::InputFormat;
use crate::model::{Annotation, LyricDocument};
use crate::Result;

pub mod apple_music;
pub mod experimental;
pub mod joysound;
pub mod kugou;
pub mod lrclib;
pub mod migu;
pub mod musixmatch;
pub mod netease;
pub mod offline_db;
pub mod petit_lyrics;
pub mod qq;
pub mod spotify_lyrics;
pub mod utaten;
pub mod web_sources;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum Source {
    AppleMusic,
    Awa,
    Azlyrics,
    #[value(help = "Browser-captured Musixmatch research source (experimental)")]
    BrowserMxm,
    Animesongz,
    Genius,
    #[value(alias = "joy-sound", help = "JOYSOUND Web (alias: joy-sound)")]
    Joysound,
    #[value(
        alias = "j-lyric-net",
        help = "J-Lyric.net public web source (alias: j-lyric-net)"
    )]
    JLyric,
    JTotal,
    Kashinavi,
    #[value(
        alias = "kkbox-web",
        help = "KKBOX public web source (alias: kkbox-web)"
    )]
    Kkbox,
    Kugou,
    #[value(alias = "line", help = "LINE MUSIC H5 source (alias: line)")]
    LineMusic,
    #[value(alias = "lrc-lib", help = "LRCLIB (alias: lrc-lib)")]
    Lrclib,
    LyricalNonsense,
    #[value(
        alias = "migu-music",
        help = "Migu Music H5 source (alias: migu-music)"
    )]
    Migu,
    Musixmatch,
    Netease,
    #[value(
        alias = "local-db",
        help = "Local SQLite lyric database (experimental; alias: local-db)"
    )]
    OfflineDb,
    #[value(alias = "petitlyrics", help = "PetitLyrics (alias: petitlyrics)")]
    PetitLyrics,
    Qq,
    #[value(
        name = "rocklyric",
        alias = "rock-lyric",
        help = "RockLyric public web source (alias: rock-lyric)"
    )]
    RockLyric,
    Songtexte,
    #[value(
        alias = "spotify",
        help = "Spotify/Musixmatch-backed lyrics (experimental; alias: spotify)"
    )]
    SpotifyLyrics,
    #[value(
        name = "tunecore",
        alias = "linkco-re",
        help = "TuneCore/LinkCore public web source (alias: linkco-re)"
    )]
    TuneCore,
    #[value(alias = "utanet", help = "Uta-Net public web source (alias: utanet)")]
    UtaNet,
    #[value(alias = "uta-ten", help = "UtaTen (alias: uta-ten)")]
    Utaten,
    Utamap,
}

impl Source {
    pub fn is_experimental(self) -> bool {
        matches!(
            self,
            Source::BrowserMxm | Source::OfflineDb | Source::SpotifyLyrics
        )
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Source::AppleMusic => "apple-music",
            Source::Animesongz => "animesongz",
            Source::Awa => "awa",
            Source::Azlyrics => "azlyrics",
            Source::BrowserMxm => "browser-mxm",
            Source::Genius => "genius",
            Source::JLyric => "j-lyric",
            Source::JTotal => "j-total",
            Source::Joysound => "joysound",
            Source::Kashinavi => "kashinavi",
            Source::Kkbox => "kkbox",
            Source::Kugou => "kugou",
            Source::LineMusic => "line-music",
            Source::Lrclib => "lrclib",
            Source::LyricalNonsense => "lyrical-nonsense",
            Source::Migu => "migu",
            Source::Musixmatch => "musixmatch",
            Source::Netease => "netease",
            Source::OfflineDb => "offline-db",
            Source::PetitLyrics => "petit-lyrics",
            Source::Qq => "qq",
            Source::RockLyric => "rocklyric",
            Source::Songtexte => "songtexte",
            Source::SpotifyLyrics => "spotify-lyrics",
            Source::TuneCore => "tunecore",
            Source::UtaNet => "uta-net",
            Source::Utaten => "utaten",
            Source::Utamap => "utamap",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub source: Source,
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub extra: serde_json::Value,
}

impl std::fmt::Display for SearchResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let duration = self
            .duration_ms
            .map(format_duration)
            .unwrap_or_else(|| "--:--".to_string());
        let album = self.album.as_deref().unwrap_or("-");
        write!(
            formatter,
            "{} - {} [{}] ({album})",
            self.title, self.artist, duration
        )
    }
}

#[derive(Debug, Clone)]
pub struct FetchedLyric {
    pub input_format: InputFormat,
    pub raw: Vec<u8>,
    pub document: Option<LyricDocument>,
    pub annotations: Vec<Annotation>,
}

#[async_trait]
pub trait LyricProvider: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>>;
    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProviderOptions {
    pub allow_experimental: bool,
}

pub fn provider_for(source: Source, cookie: Option<String>) -> Result<Box<dyn LyricProvider>> {
    provider_for_with_options(source, cookie, ProviderOptions::default())
}

pub fn provider_for_with_options(
    source: Source,
    cookie: Option<String>,
    options: ProviderOptions,
) -> Result<Box<dyn LyricProvider>> {
    if source.is_experimental() && !options.allow_experimental && !env_allows_experimental() {
        return Err(crate::Error::Provider(format!(
            "{} is an experimental source; pass --allow-experimental or set ROSETTRISM_ALLOW_EXPERIMENTAL=1 to use it",
            source.cli_name()
        )));
    }

    match source {
        Source::AppleMusic => Ok(Box::new(apple_music::AppleMusicProvider::new(cookie)?)),
        Source::Animesongz
        | Source::Awa
        | Source::Azlyrics
        | Source::Genius
        | Source::JLyric
        | Source::JTotal
        | Source::Kashinavi
        | Source::Kkbox
        | Source::LineMusic
        | Source::LyricalNonsense
        | Source::RockLyric
        | Source::Songtexte
        | Source::TuneCore
        | Source::UtaNet
        | Source::Utamap => web_sources::provider_for(source, cookie),
        Source::BrowserMxm => Ok(Box::new(experimental::UnsupportedExperimentalProvider::new(
            source,
            "BrowserMxm",
            "browser network capture is listed for research only and is not automated in the core CLI",
        ))),
        Source::Joysound => Ok(Box::new(joysound::JoysoundProvider::new(cookie)?)),
        Source::Kugou => Ok(Box::new(kugou::KugouProvider::new(cookie)?)),
        Source::Lrclib => Ok(Box::new(lrclib::LrclibProvider::new(cookie)?)),
        Source::Migu => Ok(Box::new(migu::MiguProvider::new(cookie)?)),
        Source::Musixmatch => Ok(Box::new(musixmatch::MusixmatchProvider::new(cookie)?)),
        Source::Netease => Ok(Box::new(netease::NeteaseProvider::new(cookie)?)),
        Source::OfflineDb => Ok(Box::new(offline_db::OfflineDbProvider::new(cookie)?)),
        Source::PetitLyrics => Ok(Box::new(petit_lyrics::PetitLyricsProvider::new(cookie)?)),
        Source::Qq => Ok(Box::new(qq::QqProvider::new(cookie)?)),
        Source::SpotifyLyrics => Ok(Box::new(spotify_lyrics::SpotifyLyricsProvider::new(
            cookie,
        )?)),
        Source::Utaten => Ok(Box::new(utaten::UtatenProvider::new(cookie)?)),
    }
}

fn env_allows_experimental() -> bool {
    std::env::var("ROSETTRISM_ALLOW_EXPERIMENTAL")
        .or_else(|_| std::env::var("LRC_DECODE_ALLOW_EXPERIMENTAL"))
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}

fn format_duration(ms: u32) -> String {
    let total_seconds = ms / 1_000;
    format!("{:02}:{:02}", total_seconds / 60, total_seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experimental_sources_are_gated_by_default() {
        let err = match provider_for_with_options(
            Source::SpotifyLyrics,
            Some("token".into()),
            ProviderOptions::default(),
        ) {
            Ok(_) => panic!("experimental provider should be gated"),
            Err(err) => err.to_string(),
        };

        assert!(err.contains("experimental source"));
        assert!(err.contains("--allow-experimental"));
    }

    #[test]
    fn allow_experimental_reaches_provider_construction() {
        let provider = provider_for_with_options(
            Source::SpotifyLyrics,
            Some("token".into()),
            ProviderOptions {
                allow_experimental: true,
            },
        );

        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn stub_sources_report_research_boundary_when_allowed() {
        let provider = provider_for_with_options(
            Source::BrowserMxm,
            None,
            ProviderOptions {
                allow_experimental: true,
            },
        )
        .unwrap();
        let err = provider.search("Song").await.unwrap_err().to_string();

        assert!(err.contains("experimental source"));
        assert!(err.contains("not implemented yet"));
    }
}
