use std::process::exit;

use anyhow::Result;

mod cli;
mod extractor;
mod local;
mod remote;
mod target;

async fn determine_teamspeak_versions(
    config: &cli::Config,
    http: &reqwest::Client,
) -> Result<(semver::Version, semver::Version)> {
    println!("⏳ Checking for updates...");
    let (last_installed_version, last_published_version) = tokio::try_join!(
        local::installed_version(config),
        remote::latest_version(config, http)
    )?;
    println!();

    Ok((last_installed_version, last_published_version))
}

#[tokio::main]
async fn main() -> Result<()> {
    let config: cli::Config = argh::from_env();
    let http = reqwest::Client::new();

    cli::print_header();
    config.print_summary();

    let (installed_version, published_version) =
        determine_teamspeak_versions(&config, &http).await?;

    if installed_version < published_version {
        println!(
            "⚠️ Update available - local {}, remote {}",
            installed_version, published_version
        );

        let server_archive = remote::download_release(&config, &http, &published_version).await?;
        local::extract_archive(server_archive, &config, &published_version).await?;
        local::swap_link(&config, &published_version).await?;

        println!();
        println!("✅ TeamSpeak successfully updated! ✅");
    } else {
        println!("✅ You are running the newest version of TeamSpeak.");
        exit(1);
    }

    Ok(())
}
