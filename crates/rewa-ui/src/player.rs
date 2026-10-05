//! Clip playback through GTK's media stream (GStreamer underneath), with the
//! calls the Windows Media Foundation player offers the interface.

use std::path::Path;

use gtk::prelude::*;
use gtk::{gdk, gio};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlayerSnapshot {
    pub ready: bool,
    pub playing: bool,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub aspect_ratio: f32,
    pub video_width: u32,
    pub video_height: u32,
}

pub struct Player {
    media: gtk::MediaFile,
    loaded: bool,
}

const MICROSECONDS: f64 = 1_000_000.0;

impl Player {
    pub fn new() -> Self {
        Self {
            media: gtk::MediaFile::new(),
            loaded: false,
        }
    }

    /// The video as GTK paints it; its contents change as frames arrive.
    pub fn paintable(&self) -> gdk::Paintable {
        self.media.clone().upcast()
    }

    pub fn media(&self) -> &gtk::MediaFile {
        &self.media
    }

    /// Opens a clip and starts it as soon as it is ready, like the Windows player.
    pub fn open(&mut self, path: &Path, volume_percent: u8) -> Result<(), String> {
        self.media.set_file(Some(&gio::File::for_path(path)));
        self.media
            .set_volume(f64::from(volume_percent.min(100)) / 100.0);
        self.media.play();
        self.loaded = true;
        match self.media.error() {
            Some(error) => Err(error.to_string()),
            None => Ok(()),
        }
    }

    pub fn close(&mut self) {
        if !self.loaded {
            return;
        }
        self.media.pause();
        self.media.clear();
        self.loaded = false;
    }

    fn loaded(&self) -> Result<(), String> {
        if let Some(error) = self.media.error() {
            return Err(error.to_string());
        }
        if self.loaded {
            Ok(())
        } else {
            Err("No clip is loaded".into())
        }
    }

    pub fn toggle(&self) -> Result<(), String> {
        self.loaded()?;
        if !self.media.is_prepared() {
            return Ok(());
        }
        if self.media.is_playing() {
            self.media.pause();
        } else {
            let snapshot = self.snapshot();
            if self.media.is_ended()
                || (snapshot.duration_seconds > 0.0
                    && snapshot.position_seconds + 0.05 >= snapshot.duration_seconds)
            {
                self.seek_fraction(0.0)?;
            }
            self.media.play();
        }
        Ok(())
    }

    pub fn play(&self) -> Result<(), String> {
        self.loaded()?;
        self.media.play();
        Ok(())
    }

    pub fn seek_fraction(&self, fraction: f64) -> Result<(), String> {
        self.loaded()?;
        if !self.media.is_prepared() || !self.media.is_seekable() {
            return Ok(());
        }
        let duration = self.media.duration();
        self.media
            .seek((duration as f64 * fraction.clamp(0.0, 1.0)).round() as i64);
        Ok(())
    }

    pub fn set_volume(&self, percent: u8) -> Result<(), String> {
        self.media.set_volume(f64::from(percent.min(100)) / 100.0);
        Ok(())
    }

    pub fn snapshot(&self) -> PlayerSnapshot {
        if !self.loaded {
            return PlayerSnapshot::default();
        }
        let ready = self.media.is_prepared();
        let duration_seconds = self.media.duration() as f64 / MICROSECONDS;
        // GStreamer reports 0 once a clip has ended; the playhead belongs at the end,
        // as on Windows, so there is something to drag back from
        let position_seconds = if self.media.is_ended() {
            duration_seconds
        } else {
            self.media.timestamp() as f64 / MICROSECONDS
        };
        let width = self.media.intrinsic_width().max(0) as u32;
        let height = self.media.intrinsic_height().max(0) as u32;
        PlayerSnapshot {
            ready,
            playing: self.media.is_playing(),
            position_seconds,
            duration_seconds,
            aspect_ratio: if width > 0 && height > 0 {
                width as f32 / height as f32
            } else {
                16.0 / 9.0
            },
            video_width: width,
            video_height: height,
        }
    }
}
