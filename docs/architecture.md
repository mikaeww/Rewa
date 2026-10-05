# Architecture

Rewa keeps the resident path intentionally small.

```text
Hyprland bind or desktop shortcut
    │
    ▼
rewactl ── Unix socket ──► rewad ── signals ──► gpu-screen-recorder
                                      │
                                      └──────────────► local clip directory

rewa-ui ── Unix socket ──► rewad   (status, save, reload)
     │
     └── writes ~/.config/rewa/config.toml, browses and trims the clip directory
```

`rewad` links only the Rust standard library, Serde, and the configuration
parser. It does not link GTK. `rewa-ui` is a separate executable, so none
of its UI dependencies are mapped into the daemon. It shares its state, text
and motion with the Windows window through `rewa-shell`; see
[Linux UI parity](linux-ui-parity.md).

On Hyprland, Rewa keeps the native runtime bind and precise focused-monitor
metadata. On Plasma and other desktops, GPU Screen Recorder supplies the
available connector names and the desktop owns a shortcut that runs
`rewactl save`. If direct connector capture is unavailable, Rewa can use the
XDG desktop portal and restore the approved screen selection from its local
cache.

The recorder engine receives a connector name such as `DP-1` or the `portal`
target, encodes with the GPU, and keeps only the configured replay duration as
compressed data. Saving sends `SIGUSR1`; the existing encoded buffer is written
without a second encode.

`rewad` checks the recorder process four times per second. A failed recorder
is restarted with a bounded backoff, while systemd independently keeps the
daemon attached to the user's `default.target`. This avoids relying on a
desktop-specific graphical-session target that may never become active.
