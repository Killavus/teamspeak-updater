# TeamSpeak Server Updater

This little piece of utility software automates the busy-work related to updating the TeamSpeak server binaries. TeamSpeak has this peculiar behavior of connecting periodically to mothership and shutting down your server the moment an update is available. Every time it happens on my server I have to perform manual downloading & extracting new binaries and restarting the related daemon runner. This tool automates the first two parts of the process.

This utility is easily replaceable by a simple bash script. I've used Rust because I like it 😛.

## Usage

For all configuration options, see `--help`.

This tool works in an opinionated way to perform its task. You need to setup your installation in a way it supports this tool.

- The current version running is a symlink to directory in a format `/path/to/teamspeak/version/<x.x.x>`. So you want to have installation of TeamSpeak as a symlink (paths are configurable) `/opt/teamspeak` pointing to `/opt/teamspeak-releases/3.13.7`.
- Tool will inspect the symlink (specified by `--symlink-path` configuration option, defaults to `/opt/teamspeak`) to determine the current version. So if `/opt/teamspeak` links to `/opt/teamspeak-releases/3.13.7`, `3.13.7` will be determined as current local version.
- If, after connecting to mirror (specified by `--mirror-url`, default is `https://files.teamspeak-services.com/releases/server/`), latest published version is higher than current local version (let's say there is `3.13.8` directory on mirror), it'll download & extract the archive suitable for your platform (configurable by `--target-tuple` option - it tries to guess though using Rust `cfg!` `target_os` / `target_arch` if not specified).
- New version will be extracted to `--releases-path` (default: `/opt/teamspeak-releases`) folder as a subfolder named `x.y.z` where `x.y.z` is a latest published version. So in case of default settings `/opt/teamspeak-releases/3.13.8` for latest published version `3.13.8`.
- New symlink will get created pointing to the newest release. Old symlink will get renamed to `<old_symlink_name>.<timestamp>` so you can easily restore your previous setup in case something goes wrong. So after updating `--symlink-path` will point to the latest published version directory.

You need to configure your environment so the user running this program has all required accesses. On Windows, remember that creating symlinks by default requires administrator priviledges. If target release directory exists, all files within will get overwritten. Tool does not run if it does not detect that current local version is lower than latest published version, so in this case nothing will get overwritten.

## Installation

You need to have [Rust toolchain](https://rustup.rs/) installed.

```
git clone https://github.com/Killavus/teamspeak-updater.git
cd teamspeak-updater
cargo build --release
# needs root:
cp target/release/teamspeak-updater /usr/local/bin
```

Pre-built binaries _may_ be supplied in the future.

## License

[Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0)
