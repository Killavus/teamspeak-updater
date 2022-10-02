use std::path::PathBuf;

use anyhow::Result;
use argh::FromArgs;

mod target {
    use std::{fmt::Display, str::FromStr};
    use thiserror::Error;

    pub enum Tuple {
        WindowsX86,
        WindowsX8664,
        LinuxX8664,
        Mac,
        FreeBSDX8664,
        LinuxAlpine,
        LinuxX86,
    }

    #[derive(Debug, Error)]
    pub enum TupleError {
        #[error("target tuple not recognized: {0}")]
        NotRecognized(String),
    }

    enum ArchiveType {
        Bzip2Tarball,
        Zip,
    }

    impl ArchiveType {
        fn extension(&self) -> &'static str {
            match &self {
                Self::Bzip2Tarball => "tar.bz2",
                Self::Zip => "zip",
            }
        }
    }

    impl Display for ArchiveType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.extension())
        }
    }

    impl FromStr for Tuple {
        type Err = TupleError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "linux_amd64" => Ok(Self::LinuxX8664),
                "win64" => Ok(Self::WindowsX8664),
                "linux_alpine" => Ok(Self::LinuxAlpine),
                "freebsd_amd64" => Ok(Self::FreeBSDX8664),
                "mac" => Ok(Self::Mac),
                "win32" => Ok(Self::WindowsX86),
                "linux_x86" => Ok(Self::LinuxX86),
                _ => Err(TupleError::NotRecognized(s.to_owned())),
            }
        }
    }

    impl Tuple {
        fn target_string(&self) -> &'static str {
            match &self {
                Self::LinuxAlpine => "linux_alpine",
                Self::LinuxX86 => "linux_x86",
                Self::FreeBSDX8664 => "freebsd_amd64",
                Self::LinuxX8664 => "linux_amd64",
                Self::Mac => "mac",
                Self::WindowsX86 => "win32",
                Self::WindowsX8664 => "win64",
            }
        }

        pub fn archive_filename(&self, version: &semver::Version) -> String {
            format!(
                "teamspeak3-server_{}-{}.{}",
                self.target_string(),
                version,
                self.archive_type().extension()
            )
        }

        fn archive_type(&self) -> ArchiveType {
            match &self {
                Self::Mac | Self::WindowsX86 | Self::WindowsX8664 => ArchiveType::Zip,
                _ => ArchiveType::Bzip2Tarball,
            }
        }

        pub fn deduce() -> Self {
            let tuple_str = if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
                "win64"
            } else if cfg!(all(target_os = "windows", target_arch = "x86")) {
                "win32"
            } else if cfg!(target_os = "macos") {
                "mac"
            } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
                "linux_amd64"
            } else if cfg!(all(target_os = "linux", target_arch = "x86")) {
                "linux_x86"
            } else if cfg!(target_os = "freebsd") {
                "freebsd_amd64"
            } else {
                "not supported"
            };

            if let Ok(tuple) = Self::from_str(tuple_str) {
                tuple
            } else {
                panic!("failed to deduce target tuple - you need to provide it by yourself.");
            }
        }
    }

    impl Display for Tuple {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.target_string())
        }
    }
}

/// Check for update and install new TeamSpeak version, automatically.
#[derive(FromArgs)]
pub struct Config {
    /// path to TeamSpeak symlink which will be used for pinning the latest version.
    #[argh(option, default = "PathBuf::from(\"/opt/teamspeak\")")]
    symlink_path: PathBuf,
    /// path to releases directory where all downloaded TeamSpeak versions will be stored.
    #[argh(option, default = "PathBuf::from(\"/opt/teamspeak-releases\")")]
    releases_path: PathBuf,
    /// operating system / architecture tuple used to recognize which TeamSpeak version should be installed.
    #[argh(option, default = "target::Tuple::deduce()")]
    target_tuple: target::Tuple,
    /// mirror from where TeamSpeak version should be matched.
    #[argh(
        option,
        default = "String::from(\"https://files.teamspeak-services.com/releases/server/\")"
    )]
    mirror_url: String,
}

mod teamspeak_local {
    use crate::Config;
    use anyhow::Result;
    use semver::Version;

    pub async fn installed_version(config: &Config) -> Result<Version> {
        let Config { symlink_path, .. } = config;
        use anyhow::anyhow;
        use tokio::fs;

        let real_path = fs::canonicalize(&symlink_path).await?;
        if real_path.is_dir() {
            let version_path = real_path.file_name().and_then(|name| name.to_str());

            match version_path {
                Some(version_path) => Ok(Version::parse(version_path).map(|version| {
                    println!(
                        "🏠 Determined locally installed TeamSpeak version: {}",
                        version
                    );
                    version
                })?),
                None => Err(anyhow!(
                    "Directory the symlink is pointing to is not valid UTF-8"
                )),
            }
        } else {
            Err(anyhow!("Path symlink is pointing to is not a directory"))
        }
    }

    pub async fn extract_archive(
        server_archive: tokio::fs::File,
        config: &Config,
        published_version: &semver::Version,
    ) -> Result<()> {
        Ok(())
    }
}

mod teamspeak_remote {
    use crate::Config;
    use anyhow::{anyhow, Result};
    use reqwest::Client;
    use scraper::{Html, Selector};
    use semver::Version;

    fn versions(listing_body: String) -> Vec<Version> {
        let fragment = Html::parse_fragment(&listing_body);
        let selector = Selector::parse("pre > a").expect("selector is invalid");

        let mut versions = vec![];

        for version_link in fragment.select(&selector) {
            let version_text =
                version_link
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

        let mut tempfile =
            tokio::io::BufWriter::new(tokio::fs::File::from_std(tempfile::tempfile()?));

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
}

fn print_config_summary(config: &Config) {
    println!("🔧 Configuration Summary");
    println!(
        "Symlink of current TeamSpeak directory: {}",
        config.symlink_path.to_string_lossy()
    );
    println!(
        "Directory containing TeamSpeak releases: {}",
        config.releases_path.to_string_lossy()
    );
    println!(
        "Mirror URL used to check for TeamSpeak versions: {}",
        config.mirror_url
    );
    println!();
}

fn print_header() {
    println!(
        "🚀 TeamSpeak Auto-Updater v{} 🚀",
        env!("CARGO_PKG_VERSION")
    );
    println!()
}

async fn determine_teamspeak_versions(
    config: &Config,
    http: &reqwest::Client,
) -> Result<(semver::Version, semver::Version)> {
    println!("⏳ Checking for updates...");
    let (last_installed_version, last_published_version) = tokio::try_join!(
        teamspeak_local::installed_version(config),
        teamspeak_remote::latest_version(config, http)
    )?;
    println!();

    Ok((last_installed_version, last_published_version))
}

#[tokio::main]
async fn main() -> Result<()> {
    let config: Config = argh::from_env();
    let http = reqwest::Client::new();

    print_header();
    print_config_summary(&config);

    let (installed_version, published_version) =
        determine_teamspeak_versions(&config, &http).await?;

    if installed_version < published_version {
        println!(
            "⚠️ Update available - local {}, remote {}",
            installed_version, published_version
        );

        let server_archive =
            teamspeak_remote::download_release(&config, &http, &published_version).await?;
        teamspeak_local::extract_archive(server_archive, &config, &published_version).await?;
    }

    Ok(())
}
