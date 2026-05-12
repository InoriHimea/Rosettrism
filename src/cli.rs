use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand, ValueEnum};
use dialoguer::Select;
use tokio::fs;

use crate::decoder::{decode_bytes, decode_raw_bytes, detect_format, InputFormat};
use crate::exporter::{export_document, OutputFormat};
use crate::provider::{LyricProvider, ProviderOptions, SearchResult, Source};
use crate::service::{
    parse_ttl, AggregateFetchRequest, LyricNeed, MergeMode, ServiceContext, SpecificFetchFormat,
    SpecificFetchResult,
};
use crate::{cache::UpstreamCache, server};

#[derive(Debug, Parser)]
#[command(
    name = "Rosettrism",
    version,
    about = "Rosettrism (rosettrism/rstm/rosm) decodes local lyrics and fetches online KRC/QRC/YRC/TTML lyrics"
)]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    cookie_file: Option<PathBuf>,

    #[arg(long, global = true)]
    allow_experimental: bool,

    #[arg(long, global = true, value_name = "PATH")]
    offline_db: Option<PathBuf>,

    #[arg(long, global = true, value_name = "PATH")]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Decode a local lyric file")]
    Decode {
        #[arg(value_name = "PATH")]
        input: PathBuf,

        #[arg(short = 'f', long = "format", value_enum, default_value = "lrc")]
        format: OutputFormat,

        #[arg(short = 'o', long = "output", value_name = "PATH")]
        output: Option<PathBuf>,

        #[arg(long = "input-format", value_enum, default_value = "auto")]
        input_format: InputFormat,
    },
    #[command(about = "Search lyrics, choose a candidate, and download the raw lyric file")]
    Search {
        #[arg(value_name = "QUERY")]
        query: String,

        #[arg(long = "source", value_enum)]
        source: Option<Source>,

        #[arg(short = 'o', long = "output", value_name = "PATH")]
        output: Option<PathBuf>,

        #[arg(long = "ttl", default_value = "7d")]
        ttl: String,

        #[arg(long = "force-refresh")]
        force_refresh: bool,
    },
    #[command(about = "Fetch source-specific raw/json lyrics or aggregate unified JSON")]
    Fetch {
        #[arg(value_name = "QUERY")]
        query: String,

        #[arg(long = "source", value_enum)]
        source: Option<Source>,

        #[arg(short = 'f', long = "format", value_enum)]
        format: Option<FetchOutputFormat>,

        #[arg(short = 'o', long = "output", value_name = "PATH")]
        output: Option<PathBuf>,

        #[arg(long = "merge-mode", value_enum, default_value = "tracks")]
        merge_mode: MergeMode,

        #[arg(long = "top")]
        top: Option<usize>,

        #[arg(long = "needs")]
        needs: Option<String>,

        #[arg(long = "translation-lang", default_value = "zh-Hans")]
        translation_lang: String,

        #[arg(long = "ttl", default_value = "7d")]
        ttl: String,

        #[arg(long = "force-refresh")]
        force_refresh: bool,
    },
    #[command(about = "Run the local HTTP API and embedded dashboard")]
    Server {
        #[arg(long = "host", default_value = "127.0.0.1")]
        host: String,

        #[arg(long = "port", default_value_t = 8080)]
        port: u16,

        #[arg(long = "open")]
        open: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FetchOutputFormat {
    Raw,
    Json,
}

impl From<FetchOutputFormat> for SpecificFetchFormat {
    fn from(format: FetchOutputFormat) -> Self {
        match format {
            FetchOutputFormat::Raw => SpecificFetchFormat::Raw,
            FetchOutputFormat::Json => SpecificFetchFormat::Json,
        }
    }
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let Cli {
        cookie_file,
        allow_experimental,
        offline_db,
        db,
        command,
    } = cli;
    let provider_options = ProviderOptions { allow_experimental };

    match command {
        Command::Decode {
            input,
            format,
            output,
            input_format,
        } => {
            let bytes = fs::read(&input)
                .await
                .with_context(|| format!("failed to read {}", input.display()))?;
            let detected = if input_format == InputFormat::Auto {
                detect_format(&bytes)
            } else {
                input_format
            };
            let rendered = if format == OutputFormat::Raw {
                decode_raw_bytes(&bytes, input_format)?
            } else {
                let doc = decode_bytes(&bytes, input_format)?;
                export_document(&doc, format)?
            };
            let output =
                output.unwrap_or_else(|| default_decode_output_path(&input, format, detected));
            write_output(Some(&output), &rendered).await?;
        }
        Command::Search {
            query,
            source,
            output,
            ttl,
            force_refresh,
        } => {
            let cookie = load_cookie(cookie_file.as_ref()).await?;
            let ttl = parse_ttl(&ttl)?;
            let context = service_context(
                cookie,
                offline_db,
                db.as_ref(),
                provider_options,
                ttl,
                force_refresh,
            )?;
            if let Some(source) = source {
                let provider = context.provider(source, ttl, force_refresh).await?;
                let result = search_and_select(provider.as_ref(), &query).await?;
                let fetched = provider.fetch(&result).await?;
                let output = output
                    .unwrap_or_else(|| default_download_output_path(&result, fetched.input_format));
                write_output(Some(&output), &fetched.raw).await?;
                eprintln!("Downloaded raw lyric to {}", output.display());
            } else {
                let response = context
                    .aggregate_fetch(AggregateFetchRequest {
                        query,
                        merge_mode: MergeMode::Tracks,
                        top: 5,
                        needs: LyricNeed::parse_list(None),
                        translation_lang: "zh-Hans".into(),
                        sources: None,
                        force: force_refresh,
                        ttl_seconds: Some(ttl.as_secs()),
                        ai_scoring: None,
                    })
                    .await?;
                let rendered = serde_json::to_vec_pretty(&response)?;
                write_output(output.as_ref(), &rendered).await?;
            }
        }
        Command::Fetch {
            query,
            source,
            format,
            output,
            merge_mode,
            top,
            needs,
            translation_lang,
            ttl,
            force_refresh,
        } => {
            let cookie = load_cookie(cookie_file.as_ref()).await?;
            let ttl = parse_ttl(&ttl)?;
            let context = service_context(
                cookie,
                offline_db,
                db.as_ref(),
                provider_options,
                ttl,
                force_refresh,
            )?;
            if let Some(source) = source {
                if format.is_none() && top.is_none() {
                    let response = context
                        .search_source_specific(source, &query, Some(ttl), force_refresh)
                        .await?;
                    if output.is_some() {
                        let rendered = serde_json::to_vec_pretty(&response)?;
                        write_output(output.as_ref(), &rendered).await?;
                    } else {
                        let rendered = render_results_table(&response.results);
                        write_output(None, rendered.as_bytes()).await?;
                    }
                    return Ok(());
                }
                let Some(format) = format else {
                    return Err(anyhow!(
                        "source-specific fetch requires --format raw or --format json"
                    ));
                };
                let result = context
                    .fetch_source_specific(
                        source,
                        &query,
                        format.into(),
                        top.unwrap_or(1),
                        Some(ttl),
                        force_refresh,
                    )
                    .await?;
                match result {
                    SpecificFetchResult::Raw { raw, .. } => {
                        write_output(output.as_ref(), &raw).await?;
                    }
                    SpecificFetchResult::Json { document, .. } => {
                        let rendered = export_document(&document, OutputFormat::Json)?;
                        write_output(output.as_ref(), &rendered).await?;
                    }
                    SpecificFetchResult::RawMany {
                        source,
                        results,
                        warnings,
                    } => {
                        let rendered = serde_json::to_vec_pretty(&serde_json::json!({
                            "source": source,
                            "format": "raw",
                            "results": results,
                            "warnings": warnings,
                        }))?;
                        write_output(output.as_ref(), &rendered).await?;
                    }
                    SpecificFetchResult::JsonMany {
                        source,
                        results,
                        warnings,
                    } => {
                        let rendered = serde_json::to_vec_pretty(&serde_json::json!({
                            "source": source,
                            "format": "json",
                            "results": results,
                            "warnings": warnings,
                        }))?;
                        write_output(output.as_ref(), &rendered).await?;
                    }
                }
            } else {
                if matches!(format, Some(FetchOutputFormat::Raw)) {
                    return Err(anyhow!(
                        "aggregate fetch returns unified JSON; pass --source with --format raw for source raw output"
                    ));
                }
                let response = context
                    .aggregate_fetch(AggregateFetchRequest {
                        query,
                        merge_mode,
                        top: top.unwrap_or(1),
                        needs: LyricNeed::parse_list(needs.as_deref()),
                        translation_lang,
                        sources: None,
                        force: force_refresh,
                        ttl_seconds: Some(ttl.as_secs()),
                        ai_scoring: None,
                    })
                    .await?;
                let rendered = serde_json::to_vec_pretty(&response)?;
                write_output(output.as_ref(), &rendered).await?;
            }
        }
        Command::Server { host, port, open } => {
            let cookie = load_cookie(cookie_file.as_ref()).await?;
            let ttl = crate::cache::default_ttl();
            let context = service_context(
                cookie,
                offline_db,
                db.as_ref(),
                provider_options,
                ttl,
                false,
            )?;
            server::run(server::ServerOptions {
                host,
                port,
                open_browser: open,
                context,
            })
            .await?;
        }
    }

    Ok(())
}

async fn load_cookie(cookie_file: Option<&PathBuf>) -> anyhow::Result<Option<String>> {
    if let Some(path) = cookie_file {
        let cookie = fs::read_to_string(path)
            .await
            .with_context(|| format!("failed to read cookie file {}", path.display()))?;
        return Ok(Some(cookie.trim().to_string()));
    }

    Ok(None)
}

fn print_results(results: &[crate::provider::SearchResult]) {
    eprintln!("{}", render_results_table(results));
}

fn service_context(
    cookie: Option<String>,
    offline_db: Option<PathBuf>,
    db: Option<&PathBuf>,
    provider_options: ProviderOptions,
    ttl: Duration,
    force_refresh: bool,
) -> anyhow::Result<ServiceContext> {
    let cache = match db {
        Some(path) => UpstreamCache::open(path)?,
        None => UpstreamCache::open_default()?,
    };
    Ok(ServiceContext {
        cache: Some(cache),
        provider_options,
        cookie,
        offline_db,
        default_ttl: ttl,
        force_refresh,
    })
}

async fn search_and_select(
    provider: &dyn LyricProvider,
    query: &str,
) -> anyhow::Result<SearchResult> {
    let results = provider.search(query).await?;
    if results.is_empty() {
        return Err(anyhow!("no lyric candidates found"));
    }

    print_results(&results);
    let labels = render_result_rows(&results);
    let selection = Select::new()
        .with_prompt("Choose lyric")
        .items(&labels)
        .default(0)
        .interact()
        .context("failed to read selection")?;

    Ok(results[selection].clone())
}

fn render_results_table(results: &[SearchResult]) -> String {
    let table = result_table(results);
    let mut lines = Vec::with_capacity(table.rows.len() + 2);
    lines.push(format_result_row(&table.headers, &table.widths));
    lines.push(format_separator(&table.widths));
    lines.extend(
        table
            .rows
            .iter()
            .map(|row| format_result_row(row, &table.widths)),
    );
    lines.join("\n")
}

fn render_result_rows(results: &[SearchResult]) -> Vec<String> {
    let table = result_table(results);
    table
        .rows
        .iter()
        .map(|row| format_result_row(row, &table.widths))
        .collect()
}

struct ResultTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    widths: Vec<usize>,
}

fn result_table(results: &[SearchResult]) -> ResultTable {
    let headers = ["No.", "Source", "Title", "Artist", "Album", "ID", "Time"]
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let rows = results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            vec![
                (index + 1).to_string(),
                result.source.cli_name().to_string(),
                truncate_display(&result.title, 32),
                truncate_display(&result.artist, 24),
                truncate_display(result.album.as_deref().unwrap_or("-"), 24),
                truncate_display(&result.id, 18),
                result
                    .duration_ms
                    .map(format_duration_ms)
                    .unwrap_or_else(|| "--:--".to_string()),
            ]
        })
        .collect::<Vec<_>>();
    let mut widths = headers
        .iter()
        .map(|value| display_width(value))
        .collect::<Vec<_>>();

    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(display_width(cell));
        }
    }

    ResultTable {
        headers,
        rows,
        widths,
    }
}

fn format_result_row(row: &[String], widths: &[usize]) -> String {
    row.iter()
        .zip(widths.iter().copied())
        .map(|(cell, width)| pad_display(cell, width))
        .collect::<Vec<_>>()
        .join("  ")
}

fn format_separator(widths: &[usize]) -> String {
    widths
        .iter()
        .map(|width| "-".repeat(*width))
        .collect::<Vec<_>>()
        .join("  ")
}

fn truncate_display(value: &str, max_width: usize) -> String {
    if display_width(value) <= max_width {
        return value.to_string();
    }

    let suffix = "...";
    let suffix_width = display_width(suffix);
    let mut rendered = String::new();
    let mut width = 0usize;
    for ch in value.chars() {
        let char_width = char_display_width(ch);
        if width + char_width + suffix_width > max_width {
            break;
        }
        rendered.push(ch);
        width += char_width;
    }
    rendered.push_str(suffix);
    rendered
}

fn pad_display(value: &str, width: usize) -> String {
    let mut rendered = value.to_string();
    let current = display_width(value);
    if current < width {
        rendered.push_str(&" ".repeat(width - current));
    }
    rendered
}

fn display_width(value: &str) -> usize {
    value.chars().map(char_display_width).sum()
}

fn char_display_width(ch: char) -> usize {
    if ch.is_control() {
        0
    } else if is_wide_char(ch) {
        2
    } else {
        1
    }
}

fn is_wide_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1100..=0x115f
            | 0x2329..=0x232a
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe10..=0xfe19
            | 0xfe30..=0xfe6f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
            | 0x20000..=0x3fffd
    )
}

fn format_duration_ms(ms: u32) -> String {
    let total_seconds = ms / 1_000;
    format!("{:02}:{:02}", total_seconds / 60, total_seconds % 60)
}

async fn write_output(output: Option<&PathBuf>, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).await?;
            }
        }
        fs::write(path, bytes)
            .await
            .with_context(|| format!("failed to write {}", path.display()))?;
    } else {
        let mut stdout = io::stdout().lock();
        stdout.write_all(bytes)?;
        stdout.flush()?;
    }

    Ok(())
}

fn default_decode_output_path(
    input: &std::path::Path,
    output_format: OutputFormat,
    input_format: InputFormat,
) -> PathBuf {
    let parent = input.parent().unwrap_or_else(|| std::path::Path::new(""));
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("lyric");
    let extension = match output_format {
        OutputFormat::Lrc => "lrc",
        OutputFormat::Json => "json",
        OutputFormat::Raw => match input_format {
            InputFormat::AppleMusic => "ttml",
            InputFormat::Json => "json",
            InputFormat::Qrc => "xml",
            InputFormat::Text => "txt",
            InputFormat::Yrc => "yrc",
            InputFormat::Krc | InputFormat::Lrc | InputFormat::Auto => "txt",
        },
    };

    parent.join(format!("{stem}_decoded.{extension}"))
}

fn default_download_output_path(result: &SearchResult, input_format: InputFormat) -> PathBuf {
    let stem = sanitize_file_stem(&format!("{} - {}", result.title, result.artist));
    PathBuf::from(format!("{stem}.{}", input_format_extension(input_format)))
}

fn input_format_extension(input_format: InputFormat) -> &'static str {
    match input_format {
        InputFormat::AppleMusic => "ttml",
        InputFormat::Json => "json",
        InputFormat::Krc => "krc",
        InputFormat::Qrc => "qrc",
        InputFormat::Text => "txt",
        InputFormat::Yrc => "yrc",
        InputFormat::Lrc => "lrc",
        InputFormat::Auto => "bin",
    }
}

fn sanitize_file_stem(value: &str) -> String {
    let mut stem = value
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>();

    while matches!(stem.chars().last(), Some(' ' | '.')) {
        stem.pop();
    }

    if stem.trim().is_empty() {
        "lyric".to_string()
    } else {
        stem
    }
}

#[cfg(test)]
mod tests {
    use super::{input_format_extension, sanitize_file_stem};
    use crate::decoder::InputFormat;

    #[test]
    fn maps_input_format_to_download_extension() {
        assert_eq!(input_format_extension(InputFormat::AppleMusic), "ttml");
        assert_eq!(input_format_extension(InputFormat::Json), "json");
        assert_eq!(input_format_extension(InputFormat::Krc), "krc");
        assert_eq!(input_format_extension(InputFormat::Qrc), "qrc");
        assert_eq!(input_format_extension(InputFormat::Text), "txt");
        assert_eq!(input_format_extension(InputFormat::Yrc), "yrc");
        assert_eq!(input_format_extension(InputFormat::Lrc), "lrc");
        assert_eq!(input_format_extension(InputFormat::Auto), "bin");
    }

    #[test]
    fn sanitizes_download_file_stems() {
        assert_eq!(
            sanitize_file_stem("Song:Title / Artist?"),
            "Song_Title _ Artist_"
        );
        assert_eq!(sanitize_file_stem("..."), "lyric");
    }
}
