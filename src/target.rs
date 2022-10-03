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
