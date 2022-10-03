use crate::{cli::Config, extractor};
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
