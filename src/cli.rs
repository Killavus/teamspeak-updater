use crate::target;
use argh::FromArgs;
use std::path::PathBuf;

/// Check for update and install new TeamSpeak version, automatically.
#[derive(FromArgs)]
pub struct Config {
    /// path to TeamSpeak symlink which will be used for pinning the latest version.
    #[argh(option, default = "PathBuf::from(\"/opt/teamspeak\")")]
    pub symlink_path: PathBuf,
    /// path to releases directory where all downloaded TeamSpeak versions will be stored.
    #[argh(option, default = "PathBuf::from(\"/opt/teamspeak-releases/\")")]
    pub releases_path: PathBuf,
    /// operating system / architecture tuple used to recognize which TeamSpeak version should be installed.
    #[argh(option, default = "target::Tuple::deduce()")]
    pub target_tuple: target::Tuple,
    /// mirror from where TeamSpeak version should be matched.
    #[argh(
        option,
        default = "String::from(\"https://files.teamspeak-services.com/releases/server/\")"
    )]
    pub mirror_url: String,
}

impl Config {
    pub fn print_summary(&self) {
        println!("🔧 Configuration Summary");
        println!(
            "Symlink of current TeamSpeak directory: {}",
            self.symlink_path.to_string_lossy()
        );
        println!(
            "Directory containing TeamSpeak releases: {}",
            self.releases_path.to_string_lossy()
        );
        println!(
            "Mirror URL used to check for TeamSpeak versions: {}",
            self.mirror_url
        );
        println!("Package target tuple: {}", self.target_tuple,);
        println!();
    }
}

pub fn print_header() {
    println!(
        "🚀 TeamSpeak Auto-Updater v{} 🚀",
        env!("CARGO_PKG_VERSION")
    );
    println!()
}
