# Linux UI parity

The Linux window shows the same interface as the Windows application. It is not
a second design: both draw from one model and the same layout numbers.

## How the two sides share the interface

| Part | Windows | Linux |
| --- | --- | --- |
| State, actions, filters, trim logic | `crates/rewa-shell/src/model.rs` | same file |
| German and English text | `crates/rewa-shell/src/text.rs` | same file; the few strings that name the OS differ by `cfg` |
| Springs (glide, settle, quick) | `crates/rewa-shell/src/motion.rs` | same file |
| Local calendar time | `crates/rewa-shell/src/clock.rs` | same file (`localtime_r` instead of the Windows time zone API) |
| Drawing and hit regions | `crates/rewa-win-ui/src/renderer.rs` (Direct2D) | `crates/rewa-ui/src/render/` (GTK snapshot) |
| Window, input, workers | `crates/rewa-win-ui/src/app.rs` | `crates/rewa-ui/src/app/` |

The Linux renderer is a port of the Windows renderer: pages, controls and
geometry keep their Windows names and numbers, only the primitives in
`render/painter.rs`, `render/glyph.rs` and `render/images.rs` differ. A change to
the look is made in `renderer.rs` and then in the module of the same name under
`crates/rewa-ui/src/render/`.

## Platform substitutions

| Windows | Linux |
| --- | --- |
| Segoe UI Variable | the desktop font from `gtk-font-name` |
| Media Foundation player in a child window | `gtk::MediaFile` (GStreamer) drawn into the same surface |
| Shell thumbnails and durations | `rewa_core::clips::build_preview` (ffmpeg, ffprobe) on a worker |
| Named pipe to `rewad.exe` | Unix socket to `rewad`, started with `systemctl --user` |
| Registered hotkey | Hyprland bind through `rewa_core::shortcuts`; other desktops get the `rewactl save` command to bind |
| Task Scheduler autostart | `systemctl --user enable rewad.service` |
| WASAPI microphone meter | `parec`, only while the microphone test runs |
| Explorer, shell drag and drop | GTK file launcher and `gdk::Drag` with a file list |
| Tray icon, saved-clip toast | not on Linux |

## Checking

- `cargo test --workspace` and `cargo clippy -p rewa-ui -p rewa-shell --all-targets -- -D warnings`.
- `rewa-ui --render OUT.png [clips|collections|general|recording|audio|storage|about] [dark|light|cafe|pink|candy] [folded] [capture] [filters] [english] [WxH]`
  paints one page into a PNG without a window. Under Hyprland run it as
  `env -u WAYLAND_DISPLAY GDK_BACKEND=x11 xvfb-run -a rewa-ui --render …`, otherwise
  GTK would connect to the session.
- Interactive checks run the real binary in its own Xvfb server with
  `GSK_RENDERER=cairo REWA_UI_NON_UNIQUE=1`, a copied `XDG_CONFIG_HOME`, and
  `PULSE_SERVER`/`PIPEWIRE_REMOTE` pointed at nothing so playback stays silent.

## Known gaps

- The Linux window has no accessibility tree; like the Windows renderer it draws
  everything itself.
- As on Windows, playback has no volume slider; fullscreen has a mute door.
