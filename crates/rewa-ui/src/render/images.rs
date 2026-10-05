//! Pictures: clip thumbnails and lengths made off the UI thread by ffmpeg, and the
//! ghost and app icon shipped inside the executable. Windows asks the shell for
//! the same thumbnails; here `rewa_core::clips::build_preview` makes them.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::SystemTime;

use gdk_pixbuf::Pixbuf;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use rewa_core::clips::{self, Clip, ClipPreview};

use super::*;

const GHOST_PNG: &[u8] = include_bytes!("../../../rewa-win-ui/src/ghost-64.png");
const APP_ICON: &[u8] = include_bytes!("../../../../packaging/windows/rewa.ico");
const MAX_THUMBNAILS: usize = 32;

pub(crate) struct Images {
    thumbnails: HashMap<PathBuf, gdk::Texture>,
    order: VecDeque<PathBuf>,
    /// Thumbnails drawn in the frame being painted; eviction never takes these.
    painted: HashSet<PathBuf>,
    durations: HashMap<PathBuf, Option<u64>>,
    unavailable: HashSet<PathBuf>,
    requested: HashSet<PathBuf>,
    requests: Option<mpsc::Sender<PathBuf>>,
    results: Option<mpsc::Receiver<(PathBuf, ClipPreview)>>,
    thumbnail_directory: PathBuf,
    ghost: Option<gdk::Texture>,
    app_icon: Option<gdk::Texture>,
}

impl Images {
    pub(crate) fn new(thumbnail_directory: PathBuf) -> Self {
        Self {
            thumbnails: HashMap::new(),
            order: VecDeque::new(),
            painted: HashSet::new(),
            durations: HashMap::new(),
            unavailable: HashSet::new(),
            requested: HashSet::new(),
            requests: None,
            results: None,
            thumbnail_directory,
            ghost: None,
            app_icon: None,
        }
    }

    pub(crate) fn begin_frame(&mut self) {
        self.painted.clear();
    }

    /// One worker makes previews in order, so a full grid never starts a crowd of ffmpegs.
    fn request(&mut self, path: &Path) {
        if !self.requested.insert(path.to_path_buf()) {
            return;
        }
        if self.requests.is_none() {
            let (sender, requests) = mpsc::channel::<PathBuf>();
            let (results, receiver) = mpsc::channel();
            let directory = self.thumbnail_directory.clone();
            let spawned = std::thread::Builder::new()
                .name("rewa-thumbnails".into())
                .spawn(move || {
                    for path in requests {
                        let preview = preview(&path, &directory);
                        if results.send((path, preview)).is_err() {
                            break;
                        }
                    }
                });
            if spawned.is_err() {
                self.unavailable.insert(path.to_path_buf());
                return;
            }
            self.requests = Some(sender);
            self.results = Some(receiver);
        }
        if let Some(requests) = &self.requests
            && requests.send(path.to_path_buf()).is_err()
        {
            self.requests = None;
            self.unavailable.insert(path.to_path_buf());
        }
    }

    /// Takes in finished previews; true when one changed what the grid shows.
    pub(crate) fn collect(&mut self) -> bool {
        let Some(results) = &self.results else {
            return false;
        };
        let finished = results.try_iter().collect::<Vec<_>>();
        for (path, preview) in &finished {
            self.durations
                .insert(path.clone(), preview.duration_seconds);
            let texture = preview
                .thumbnail
                .as_ref()
                .and_then(|file| Pixbuf::from_file_at_scale(file, 320, 180, true).ok())
                .map(|pixbuf| gdk::Texture::for_pixbuf(&pixbuf));
            match texture {
                Some(texture) => {
                    self.thumbnails.insert(path.clone(), texture);
                    self.order.retain(|cached| cached != path);
                    self.order.push_back(path.clone());
                }
                None => {
                    self.unavailable.insert(path.clone());
                }
            }
        }
        self.evict_cold();
        !finished.is_empty()
    }

    fn evict_cold(&mut self) {
        while self.order.len() > MAX_THUMBNAILS {
            // a grid with more cards than the cap would otherwise evict what it is
            // about to draw and make it again on every paint
            let Some(position) = self
                .order
                .iter()
                .position(|cached| !self.painted.contains(cached))
            else {
                break;
            };
            if let Some(cold) = self.order.remove(position) {
                self.thumbnails.remove(&cold);
                self.requested.remove(&cold);
            }
        }
    }

    pub(crate) fn retry_unavailable(&mut self) {
        for path in self.unavailable.drain() {
            self.requested.remove(&path);
        }
    }

    pub(crate) fn forget(&mut self, path: &Path) {
        self.thumbnails.remove(path);
        self.order.retain(|cached| cached != path);
        self.durations.remove(path);
        self.unavailable.remove(path);
        self.requested.remove(path);
    }
}

fn preview(path: &Path, thumbnail_directory: &Path) -> ClipPreview {
    let metadata = std::fs::metadata(path).ok();
    let clip = Clip {
        path: path.to_path_buf(),
        title: String::new(),
        size_bytes: metadata.as_ref().map_or(0, std::fs::Metadata::len),
        modified: metadata
            .and_then(|metadata| metadata.modified().ok())
            .unwrap_or(SystemTime::UNIX_EPOCH),
    };
    clips::build_preview(&clip, thumbnail_directory)
}

fn embedded(bytes: &'static [u8]) -> Option<gdk::Texture> {
    let loader = gdk_pixbuf::PixbufLoader::new();
    loader.write(bytes).ok()?;
    loader.close().ok()?;
    loader
        .pixbuf()
        .map(|pixbuf| gdk::Texture::for_pixbuf(&pixbuf))
}

impl Renderer {
    pub(crate) fn draw_thumbnail(
        &mut self,
        path: &Path,
        destination: LogicalRect,
        radius: f32,
    ) -> Result<bool, String> {
        self.images.painted.insert(path.to_path_buf());
        let Some(texture) = self.images.thumbnails.get(path).cloned() else {
            if !self.images.unavailable.contains(path) {
                self.images.request(path);
            }
            return Ok(false);
        };
        if let Some(position) = self.images.order.iter().position(|cached| cached == path)
            && let Some(cached) = self.images.order.remove(position)
        {
            self.images.order.push_back(cached);
        }
        self.picture(&texture, destination, radius, 1.0);
        Ok(true)
    }

    pub(crate) fn clip_duration(&mut self, path: &Path) -> Option<u64> {
        match self.images.durations.get(path) {
            Some(duration) => *duration,
            None => {
                self.images.request(path);
                None
            }
        }
    }

    pub(crate) fn draw_ghost(&mut self, area: LogicalRect, opacity: f32) {
        if self.images.ghost.is_none() {
            self.images.ghost = gdk::Texture::from_bytes(&glib::Bytes::from_static(GHOST_PNG)).ok();
        }
        if let Some(ghost) = self.images.ghost.clone() {
            self.picture(&ghost, area, 0.0, opacity);
        }
    }

    pub(crate) fn draw_app_icon(&mut self, area: LogicalRect) -> Result<(), String> {
        if self.images.app_icon.is_none() {
            self.images.app_icon = embedded(APP_ICON);
        }
        if let Some(icon) = self.images.app_icon.clone() {
            self.picture(&icon, area, 0.0, 1.0);
        }
        Ok(())
    }
}
