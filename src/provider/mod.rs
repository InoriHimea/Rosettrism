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
pub mod runtime;
pub use runtime::{ProviderRequestPolicy, ProviderRuntime};
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
        self.capabilities().experimental
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

    pub fn display_name(self) -> &'static str {
        match self {
            Source::AppleMusic => "Apple Music",
            Source::BrowserMxm => "Browser Musixmatch",
            Source::JLyric => "J-Lyric",
            Source::Joysound => "JOYSOUND",
            Source::Kkbox => "KKBOX",
            Source::LineMusic => "LINE MUSIC",
            Source::Lrclib => "LRCLIB",
            Source::Migu => "Migu Music",
            Source::Musixmatch => "Musixmatch",
            Source::Netease => "NetEase Cloud Music",
            Source::OfflineDb => "Offline DB",
            Source::PetitLyrics => "PetitLyrics",
            Source::Qq => "QQ Music",
            Source::RockLyric => "RockLyric",
            Source::SpotifyLyrics => "Spotify Lyrics",
            Source::TuneCore => "TuneCore",
            Source::UtaNet => "Uta-Net",
            Source::Utaten => "UtaTen",
            Source::Animesongz => "AnimeSongZ",
            Source::Awa => "AWA",
            Source::Azlyrics => "AZLyrics",
            Source::Genius => "Genius",
            Source::JTotal => "J-Total Music",
            Source::Kashinavi => "Kashinavi",
            Source::Kugou => "Kugou",
            Source::LyricalNonsense => "Lyrical Nonsense",
            Source::Songtexte => "Songtexte",
            Source::Utamap => "UtaMap",
        }
    }

    pub fn capabilities(self) -> ProviderCapabilities {
        let base = ProviderCapabilities::new().direct_id();
        match self {
            Source::AppleMusic => base
                .word_timing()
                .translation()
                .romanized()
                .ruby()
                .requires_cookie(),
            Source::BrowserMxm => ProviderCapabilities::new()
                .word_timing()
                .translation()
                .requires_cookie()
                .experimental(),
            Source::Joysound => base.ruby(),
            Source::Kugou => base.word_timing().translation(),
            Source::Lrclib => base,
            Source::Migu => base.translation(),
            Source::Musixmatch => base.word_timing().translation().requires_cookie(),
            Source::Netease => base.word_timing().translation().romanized(),
            Source::OfflineDb => base
                .word_timing()
                .translation()
                .romanized()
                .ruby()
                .experimental(),
            Source::PetitLyrics => base,
            Source::Qq => base.word_timing().translation().romanized(),
            Source::SpotifyLyrics => base
                .word_timing()
                .translation()
                .requires_cookie()
                .experimental(),
            Source::Utaten => base.ruby(),
            Source::Awa | Source::Kkbox | Source::LineMusic => base.requires_cookie(),
            _ => base,
        }
    }

    pub fn auth_requirement(self) -> ProviderAuthRequirement {
        match self {
            Source::OfflineDb => ProviderAuthRequirement::LocalPath,
            Source::SpotifyLyrics => ProviderAuthRequirement::RequiredToken,
            Source::AppleMusic | Source::BrowserMxm | Source::Musixmatch => {
                ProviderAuthRequirement::RequiredCookie
            }
            source if source.capabilities().requires_cookie => {
                ProviderAuthRequirement::OptionalCookie
            }
            _ => ProviderAuthRequirement::None,
        }
    }

    pub fn provider_config(self) -> ProviderConfig {
        match self {
            Source::Musixmatch | Source::SpotifyLyrics | Source::AppleMusic => ProviderConfig {
                timeout_ms: 20_000,
                retry: ProviderRetryPolicy {
                    max_retries: 1,
                    backoff_ms: 750,
                },
                rate_limit: ProviderRateLimit {
                    requests: 20,
                    per_seconds: 60,
                },
            },
            Source::OfflineDb => ProviderConfig {
                timeout_ms: 5_000,
                retry: ProviderRetryPolicy {
                    max_retries: 0,
                    backoff_ms: 0,
                },
                rate_limit: ProviderRateLimit {
                    requests: 1_000,
                    per_seconds: 60,
                },
            },
            Source::Netease | Source::Qq | Source::Kugou | Source::Migu => ProviderConfig {
                timeout_ms: 15_000,
                retry: ProviderRetryPolicy {
                    max_retries: 2,
                    backoff_ms: 500,
                },
                rate_limit: ProviderRateLimit {
                    requests: 30,
                    per_seconds: 60,
                },
            },
            _ => ProviderConfig {
                timeout_ms: 12_000,
                retry: ProviderRetryPolicy {
                    max_retries: 1,
                    backoff_ms: 500,
                },
                rate_limit: ProviderRateLimit {
                    requests: 15,
                    per_seconds: 60,
                },
            },
        }
    }

    pub fn decoder_output_format(self) -> Vec<InputFormat> {
        match self {
            Source::AppleMusic => vec![InputFormat::AppleMusic],
            Source::Kugou => vec![InputFormat::Krc],
            Source::Netease => vec![InputFormat::Yrc, InputFormat::Lrc],
            Source::Qq => vec![InputFormat::Qrc],
            Source::Lrclib | Source::Migu => vec![InputFormat::Lrc],
            Source::OfflineDb => vec![InputFormat::Json, InputFormat::Lrc, InputFormat::Text],
            _ => vec![InputFormat::Text, InputFormat::Lrc],
        }
    }

    pub fn manifest(self) -> ProviderManifest {
        ProviderManifest {
            manifest_version: 1,
            source: self,
            name: self.cli_name().to_string(),
            source_name: self.cli_name().to_string(),
            display_name: self.display_name().to_string(),
            capabilities: self.capabilities(),
            auth: self.auth_requirement(),
            decoder_output_format: self.decoder_output_format(),
            config: self.provider_config(),
        }
    }
}

pub fn builtin_provider_registry() -> Vec<ProviderManifest> {
    // 语义升级（M1）：由内置插件注册表导出；输出形状与「枚举遍历」一致。
    ProviderRegistry::builtin().manifests()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub search: bool,
    pub direct_id: bool,
    pub word_timing: bool,
    pub translation: bool,
    pub romanized: bool,
    pub ruby: bool,
    pub requires_cookie: bool,
    pub experimental: bool,
}

impl ProviderCapabilities {
    pub const fn new() -> Self {
        Self {
            search: true,
            direct_id: false,
            word_timing: false,
            translation: false,
            romanized: false,
            ruby: false,
            requires_cookie: false,
            experimental: false,
        }
    }

    pub const fn direct_id(mut self) -> Self {
        self.direct_id = true;
        self
    }
    pub const fn word_timing(mut self) -> Self {
        self.word_timing = true;
        self
    }
    pub const fn translation(mut self) -> Self {
        self.translation = true;
        self
    }
    pub const fn romanized(mut self) -> Self {
        self.romanized = true;
        self
    }
    pub const fn ruby(mut self) -> Self {
        self.ruby = true;
        self
    }
    pub const fn requires_cookie(mut self) -> Self {
        self.requires_cookie = true;
        self
    }
    pub const fn experimental(mut self) -> Self {
        self.experimental = true;
        self
    }
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthRequirement {
    None,
    OptionalCookie,
    RequiredCookie,
    RequiredToken,
    LocalPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRetryPolicy {
    pub max_retries: u8,
    pub backoff_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRateLimit {
    pub requests: u32,
    pub per_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub timeout_ms: u64,
    pub retry: ProviderRetryPolicy,
    pub rate_limit: ProviderRateLimit,
}

pub fn apply_client_timeout(
    builder: reqwest::ClientBuilder,
    timeout_ms: u64,
) -> reqwest::ClientBuilder {
    if timeout_ms == 0 {
        builder
    } else {
        builder.timeout(std::time::Duration::from_millis(timeout_ms))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderManifest {
    pub manifest_version: u8,
    pub source: Source,
    pub name: String,
    pub source_name: String,
    pub display_name: String,
    pub capabilities: ProviderCapabilities,
    pub auth: ProviderAuthRequirement,
    pub decoder_output_format: Vec<InputFormat>,
    pub config: ProviderConfig,
}

impl ProviderManifest {
    pub fn manifest_file_name() -> &'static str {
        "rosettrism-provider.json"
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

/// 一个 provider 插件 = manifest 声明 + 凭据感知的构造工厂。
///
/// 内置插件走编译期闭包包装的 match；外部插件走 WASM/进程 ABI（后续里程碑）。
/// 设计见 `docs/provider-plugin-manifest.md` §3.1。
pub trait ProviderPlugin: Send + Sync {
    fn manifest(&self) -> ProviderManifest;

    /// 凭据解析后的 provider 实例工厂。
    /// `allow_experimental` 门禁由调用方（`provider_for_with_options`）统一执行，
    /// 插件本身不做 experimental 门禁。
    fn create_provider(
        &self,
        cookie: Option<String>,
        options: &ProviderOptions,
    ) -> Result<Box<dyn LyricProvider>>;
}

/// 注册表：按 `source_name` 查找 provider 插件。
pub struct ProviderRegistry {
    plugins: Vec<std::sync::Arc<dyn ProviderPlugin>>,
    by_source_name: std::collections::HashMap<String, usize>,
}

impl ProviderRegistry {
    /// 由插件序列构建；`source_name` 重复时返回错误。
    pub fn new(plugins: Vec<std::sync::Arc<dyn ProviderPlugin>>) -> Result<Self> {
        let mut by_source_name = std::collections::HashMap::new();
        for (index, plugin) in plugins.iter().enumerate() {
            if by_source_name
                .insert(plugin.manifest().source_name, index)
                .is_some()
            {
                return Err(crate::Error::Service(format!(
                    "duplicate provider plugin source_name: {}",
                    plugin.manifest().source_name
                )));
            }
        }
        Ok(Self {
            plugins,
            by_source_name,
        })
    }

    /// 内置 provider 注册表（覆盖全部 `Source` 变体）。
    pub fn builtin() -> Self {
        // 构造失败属编程错误（重复 source_name），CI 立即暴露。
        Self::new(builtin_provider_plugins())
            .expect("builtin provider registry has unique source names")
    }

    pub fn lookup(&self, source_name: &str) -> Option<std::sync::Arc<dyn ProviderPlugin>> {
        self.by_source_name
            .get(source_name)
            .map(|&index| std::sync::Arc::clone(&self.plugins[index]))
    }

    /// 全部插件 manifest（与 `builtin_provider_registry()` 等价）。
    pub fn manifests(&self) -> Vec<ProviderManifest> {
        self.plugins.iter().map(|p| p.manifest()).collect()
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

/// 单个内置 Source 的薄插件包装：manifest 由 `Source::manifest()` 提供，
/// 构造委托给 `provider_for_with_options`（match 形态保留至 M2）。
struct BuiltinSourcePlugin {
    source: Source,
}

impl ProviderPlugin for BuiltinSourcePlugin {
    fn manifest(&self) -> ProviderManifest {
        self.source.manifest()
    }

    fn create_provider(
        &self,
        cookie: Option<String>,
        options: &ProviderOptions,
    ) -> Result<Box<dyn LyricProvider>> {
        provider_for_with_options(self.source, cookie, options.clone())
    }
}

fn builtin_provider_plugins() -> Vec<std::sync::Arc<dyn ProviderPlugin>> {
    use std::sync::Arc;
    Source::value_variants()
        .iter()
        .map(|&source| Arc::new(BuiltinSourcePlugin { source }) as Arc<dyn ProviderPlugin>)
        .collect()
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

    let provider_config = source.provider_config();

    match source {
        Source::AppleMusic => Ok(Box::new(apple_music::AppleMusicProvider::new(
            cookie,
            provider_config.timeout_ms,
        )?)),
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
        | Source::Utamap => web_sources::provider_for(source, cookie, provider_config.timeout_ms),
        Source::BrowserMxm => Ok(Box::new(experimental::UnsupportedExperimentalProvider::new(
            source,
            "BrowserMxm",
            "browser network capture is listed for research only and is not automated in the core CLI",
        ))),
        Source::Joysound => Ok(Box::new(joysound::JoysoundProvider::new(cookie)?)),
        Source::Kugou => Ok(Box::new(kugou::KugouProvider::new(
            cookie,
            provider_config.timeout_ms,
        )?)),
        Source::Lrclib => Ok(Box::new(lrclib::LrclibProvider::new(
            cookie,
            provider_config.timeout_ms,
        )?)),
        Source::Migu => Ok(Box::new(migu::MiguProvider::new(
            cookie,
            provider_config.timeout_ms,
        )?)),
        Source::Musixmatch => Ok(Box::new(musixmatch::MusixmatchProvider::new(
            cookie,
            provider_config.timeout_ms,
        )?)),
        Source::Netease => Ok(Box::new(netease::NeteaseProvider::new(
            cookie,
            provider_config.timeout_ms,
        )?)),
        Source::OfflineDb => Ok(Box::new(offline_db::OfflineDbProvider::new(cookie)?)),
        Source::PetitLyrics => Ok(Box::new(petit_lyrics::PetitLyricsProvider::new(cookie)?)),
        Source::Qq => Ok(Box::new(qq::QqProvider::new(
            cookie,
            provider_config.timeout_ms,
        )?)),
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
    fn metadata_registry_exposes_capabilities_and_plugin_config() {
        let registry = builtin_provider_registry();
        assert_eq!(registry.len(), Source::value_variants().len());

        let netease = registry
            .iter()
            .find(|manifest| manifest.source == Source::Netease)
            .expect("netease manifest");
        assert_eq!(netease.source_name, "netease");
        assert!(netease.capabilities.search);
        assert!(netease.capabilities.word_timing);
        assert!(netease.capabilities.translation);
        assert!(netease.config.timeout_ms > 0);

        let spotify = registry
            .iter()
            .find(|manifest| manifest.source == Source::SpotifyLyrics)
            .expect("spotify manifest");
        assert!(spotify.capabilities.experimental);
        assert_eq!(spotify.auth, ProviderAuthRequirement::RequiredToken);
        assert_eq!(
            ProviderManifest::manifest_file_name(),
            "rosettrism-provider.json"
        );
    }

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

    #[test]
    fn provider_registry_covers_all_sources_and_manifests_match_legacy() {
        let registry = ProviderRegistry::builtin();
        assert_eq!(registry.len(), Source::value_variants().len());

        for source in Source::value_variants() {
            let plugin = registry
                .lookup(source.cli_name())
                .unwrap_or_else(|| panic!("missing plugin for {}", source.cli_name()));
            assert_eq!(plugin.manifest(), source.manifest());
        }
    }

    #[test]
    fn provider_registry_manifests_equal_builtin_registry_output() {
        // M1 验收锚点：注册表导出与旧「枚举遍历」输出逐条一致。
        let registry = ProviderRegistry::builtin();
        let legacy: Vec<ProviderManifest> = Source::value_variants()
            .iter()
            .map(|source| source.manifest())
            .collect();
        assert_eq!(registry.manifests(), legacy);
    }

    #[test]
    fn provider_registry_lookup_rejects_unknown_source_name() {
        let registry = ProviderRegistry::builtin();
        assert!(registry.lookup("definitely-not-a-source").is_none());
    }

    #[test]
    fn duplicate_source_name_is_rejected() {
        let result = ProviderRegistry::new(vec![
            std::sync::Arc::new(BuiltinSourcePlugin {
                source: Source::Lrclib,
            }),
            std::sync::Arc::new(BuiltinSourcePlugin {
                source: Source::Lrclib,
            }),
        ]);
        assert!(result.is_err());
    }
}
