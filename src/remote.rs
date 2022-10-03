use crate::cli::Config;
use anyhow::{anyhow, Result};
use reqwest::Client;
use scraper::{Html, Selector};
use semver::Version;

fn versions(listing_body: String) -> Vec<Version> {
    let fragment = Html::parse_fragment(&listing_body);
    let selector = Selector::parse("pre > a").expect("selector is invalid");

    let mut versions = vec![];

    for version_link in fragment.select(&selector) {
        let version_text = version_link
            .text()
            .into_iter()
            .fold(String::new(), |mut m, piece| {
                m.push_str(piece);
                m
            });

        if let Ok(version) = Version::parse(&version_text) {
            versions.push(version);
        }
    }

    versions
}

pub async fn latest_version(config: &Config, http: &Client) -> Result<Version> {
    let Config { mirror_url, .. } = config;

    let response = http.get(mirror_url).send().await?.error_for_status()?;
    let body = response.text().await?;

    let result = versions(body)
        .into_iter()
        .max()
        .ok_or_else(|| anyhow!("no versions are collected from remote endpoint"));

    if let Ok(ref version) = result {
        println!("🌐 Determined latest remote TeamSpeak version: {}", version);
    }

    result
}

pub async fn download_release(
    config: &Config,
    http: &Client,
    target: &Version,
) -> Result<tokio::fs::File> {
    use futures::stream::TryStreamExt;
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    let archive_url = remote_archive_path(config, target);
    print!("🌐 Downloading {}... ", archive_url);
    let archive_response = http.get(archive_url).send().await?.error_for_status()?;
    let tempfile = tempfile::tempfile()?;
    let mut tempfile = tokio::io::BufWriter::new(tokio::fs::File::from_std(tempfile));

    let mut stream = tokio::io::BufReader::new(
        archive_response
            .bytes_stream()
            .map_err(|e| futures::io::Error::new(futures::io::ErrorKind::Other, e))
            .into_async_read()
            .compat(),
    );

    tokio::io::copy(&mut stream, &mut tempfile).await?;
    println!("✅");
    Ok(tempfile.into_inner())
}

fn remote_archive_path(config: &Config, target: &Version) -> reqwest::Url {
    use reqwest::Url;
    let Config {
        mirror_url,
        target_tuple,
        ..
    } = config;
    let root_url = Url::parse(mirror_url).expect("mirror url is valid URL");

    root_url
        .join(&format!("{}/", target))
        .and_then(|version_url| {
            let file_name = target_tuple.archive_filename(target);
            version_url.join(&file_name)
        })
        .expect("wrong target URL format")
}
