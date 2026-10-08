<p align="center"><img src="assets/rewa-ghost.png" width="96" alt="Rewa ghost"></p>

# Rewa

An instant replay recorder for Windows and Linux. It keeps the last few minutes in memory and saves them
as a clip when you press a key. Written in Rust.

Nothing is uploaded, there is no account and no telemetry.

[Download for Windows](https://github.com/mikaeww/Rewa/releases/latest) ·
[Linux install guide](docs/install.md) ·
[Report an issue](https://github.com/mikaeww/Rewa/issues)

> [!NOTE]
> Windows is the main platform. Linux has the same window; the tray icon and the saved-clip toast are Windows only.

## Features

- Replay from 5 seconds to 10 minutes, always the full length you set
- GPU encoding on AMD, Intel and NVIDIA, with H.264, HEVC or AV1 up to 60 fps
- Desktop sound and microphone, each with its own level
- A clip library with search, collections, rename, playback and trimming
- Trimming without re-encoding where the cut allows it
- Native on both platforms: Direct2D on Windows, GTK4 on Linux, no Electron
- Less than half the memory of Medal when idle

## Install

**Windows 10 / 11:** download the setup from the [latest release](https://github.com/mikaeww/Rewa/releases/latest)
and run it as your normal user. It is not code-signed yet, so SmartScreen will warn about it.

**Arch / CachyOS:** tested on Hyprland and KDE Plasma.

```sh
git clone https://github.com/mikaeww/Rewa.git rewa
cd rewa
./scripts/install-arch.sh --install-deps
```

## Shortcuts

| Key | Action |
| --- | --- |
| `Ctrl+Alt+R` | Save the replay on Windows |
| `Super+Shift+R` | Save the replay on Hyprland |
| `rewactl save` | Bind this to a shortcut on other desktops |

## Building

Needs Rust 1.85 or newer and the dependencies from the [install guide](docs/install.md).

```sh
cargo build --locked --workspace
cargo test --locked --workspace
```

More in [architecture](docs/architecture.md) and [Windows builds](docs/windows.md).

## License

MIT

## Thanks

**Nev** tested the Windows builds and found most of the audio and clipping bugs.
