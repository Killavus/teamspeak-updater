use std::{path::PathBuf, process::exit};

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

    pub enum ArchiveType {
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

        pub fn archive_type(&self) -> ArchiveType {
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
    #[argh(option, default = "PathBuf::from(\"/opt/teamspeak-releases/\")")]
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

mod extractor {
    use crate::target::{self, ArchiveType};
    use anyhow::Result;
    use std::{
        io::{Seek, SeekFrom},
        sync::Arc,
    };

    pub async fn extract(
        archive_type: &target::ArchiveType,
        tempdir: Arc<tempfile::TempDir>,
        server_archive: tokio::fs::File,
    ) -> Result<()> {
        match archive_type {
            ArchiveType::Zip => extract_zip(tempdir, server_archive).await?,
            ArchiveType::Bzip2Tarball => extract_tarball(tempdir, server_archive).await?,
        };

        Ok(())
    }

    async fn extract_zip(
        tempdir: Arc<tempfile::TempDir>,
        server_archive: tokio::fs::File,
    ) -> Result<()> {
        use std::io::BufReader;
        use zip::ZipArchive;
        let mut server_archive = BufReader::new(server_archive.into_std().await);
        let tempdir_ = tempdir.clone();

        tokio::task::spawn_blocking::<_, Result<()>>(move || {
            server_archive.seek(SeekFrom::Start(0))?;
            let mut archive = ZipArchive::new(server_archive)?;
            archive.extract(tempdir_.path())?;

            Ok(())
        })
        .await??;

        Ok(())
    }

    async fn extract_tarball(
        tempdir: Arc<tempfile::TempDir>,
        server_archive: tokio::fs::File,
    ) -> Result<()> {
        use bzip2::bufread::BzDecoder;
        use std::io::BufReader;

        let mut server_archive = BufReader::new(server_archive.into_std().await);
        let tempdir_ = tempdir.clone();

        tokio::task::spawn_blocking::<_, Result<()>>(move || {
            use std::io::prelude::*;
            use tar::Archive;
            server_archive.seek(std::io::SeekFrom::Start(0))?;

            let mut decoder = BzDecoder::new(server_archive);
            let mut tarball_buf = vec![];

            decoder.read_to_end(&mut tarball_buf)?;

            let mut tarball = Archive::new(tarball_buf.as_slice());
            tarball.unpack(tempdir_.path())?;

            Ok(())
        })
        .await??;

        Ok(())
    }
}

mod teamspeak_local {
    use crate::{extractor, Config};
    use anyhow::Result;
    use futures::stream::FuturesUnordered;
    use semver::Version;
    use std::{io::Error, path::PathBuf, sync::Arc};

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
        let tempdir = Arc::new(tempfile::tempdir()?);
        let archive_type = config.target_tuple.archive_type();

        print!("📦 Extracting the archive... ");
        extractor::extract(&archive_type, tempdir.clone(), server_archive).await?;
        println!("✅");

        print!("📦 Moving files to new release...");
        move_extracted_files(tempdir, config, published_version).await?;
        println!("✅");

        Ok(())
    }

    async fn move_extracted_files(
        tempdir: Arc<tempfile::TempDir>,
        config: &Config,
        published_version: &semver::Version,
    ) -> Result<()> {
        use futures::prelude::*;
        use tokio::fs;

        let Config { releases_path, .. } = config;

        let mut version_path = PathBuf::from(releases_path).canonicalize()?;
        version_path.push(published_version.to_string());

        let mut read_dir = fs::read_dir(tempdir.path()).await?;

        // Since TeamSpeak archives are always getting the main folder, we need traverse it instead.
        if let Ok(Some(entry)) = read_dir.next_entry().await {
            read_dir = fs::read_dir(entry.path()).await?;
        }

        let mut read_queue = vec![(version_path.clone(), read_dir)];
        let mut file_paths = vec![];

        let ignore_exists_error = |e: Error| {
            use std::io::ErrorKind;
            if e.kind() == ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        };

        fs::create_dir(&version_path)
            .await
            .or_else(ignore_exists_error)?;

        let version_path = version_path.canonicalize()?;

        while let Some((root_path, mut read_dir)) = read_queue.pop() {
            let mut append_dirs = vec![];
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                let metadata = entry.metadata().await?;

                if metadata.is_dir() {
                    let dir_path = {
                        let mut path = root_path.canonicalize()?;
                        path.push(entry.file_name());
                        path
                    };
                    fs::create_dir(&dir_path)
                        .await
                        .or_else(ignore_exists_error)?;
                    append_dirs.push((dir_path, fs::read_dir(entry.path()).await?));
                }

                if metadata.is_file() {
                    file_paths.push(entry.path());
                }
            }

            read_queue.extend(append_dirs.into_iter());
        }

        let mut file_copying = Box::pin(
            file_paths
                .into_iter()
                .map(|path| {
                    let relative = path
                        .strip_prefix(tempdir.path())
                        .map(|relative| relative.iter().skip(1).collect::<PathBuf>());

                    relative
                        .map(|relative| version_path.join(relative))
                        .map(|to| fs::copy(path.clone(), to))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .collect::<FuturesUnordered<_>>(),
        );

        future::try_join_all(file_copying.as_mut().iter_pin_mut()).await?;

        Ok(())
    }

    pub async fn swap_link(config: &Config, published_version: &semver::Version) -> Result<()> {
        let Config {
            releases_path,
            symlink_path,
            ..
        } = config;

        use tokio::fs;

        let symlink_file_name = symlink_path
            .file_name()
            .expect("symlink should expose filename")
            .to_str()
            .expect("symlink filename is valid utf-8");

        let mut new_path = symlink_path.clone();
        let unix_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        new_path.set_file_name(format!("{}.{}", symlink_file_name, unix_timestamp));

        let new_symlink_src = {
            let mut path = releases_path.clone().canonicalize()?;
            path.push(published_version.to_string());
            path
        };

        println!(
            "🧠 Swapping symbolic links (old saved to {})",
            &new_path.as_os_str().to_string_lossy()
        );
        fs::rename(symlink_path, new_path).await?;
        fs::symlink_dir(new_symlink_src, symlink_path).await?;

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
    println!("Package target tuple: {}", config.target_tuple,);
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
        teamspeak_local::swap_link(&config, &published_version).await?;

        println!();
        println!("✅ TeamSpeak successfully updated! ✅");
    } else {
        println!("✅ You are running the newest version of TeamSpeak.");
        exit(1);
    }

    Ok(())
}
