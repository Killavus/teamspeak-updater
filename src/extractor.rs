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
