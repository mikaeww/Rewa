#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEndpoint {
    UnixSocket(PathBuf),
    NamedPipe(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub favorites_file: PathBuf,
    pub legacy_config_files: Vec<PathBuf>,
    pub cache_dir: PathBuf,
    pub thumbnail_dir: PathBuf,
    pub log_file: PathBuf,
    pub control_endpoint: ControlEndpoint,
}

/// Rewa was called Wreath: its settings and favourites move over once, while
/// clips stay wherever the carried-over config points.
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn adopt_wreath_files(legacy_dir: &std::path::Path, config_dir: &std::path::Path) {
    for name in ["config.toml", "favorites.json"] {
        let source = legacy_dir.join(name);
        let target = config_dir.join(name);
        if target.exists() || !source.exists() {
            continue;
        }
        // best effort: a failed copy still leaves the config readable through legacy_config_files
        let _ = std::fs::create_dir_all(config_dir);
        let _ = std::fs::copy(&source, &target);
    }
}

impl AppPaths {
    pub fn discover() -> Self {
        #[cfg(target_os = "linux")]
        {
            return Self::discover_linux();
        }
        #[cfg(target_os = "windows")]
        {
            return Self::discover_windows();
        }
        #[allow(unreachable_code)]
        Self::fallback()
    }

    #[cfg(target_os = "linux")]
    fn discover_linux() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let config_root = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let runtime_root = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
                std::env::temp_dir().join(format!("rewa-{user}"))
            });
        let cache_root = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"));
        let config_dir = config_root.join("rewa");
        let cache_dir = cache_root.join("rewa");
        adopt_wreath_files(&config_root.join("wreath"), &config_dir);
        Self {
            config_file: config_dir.join("config.toml"),
            favorites_file: config_dir.join("favorites.json"),
            legacy_config_files: ["wreath", "trace", "riftclip"]
                .map(|name| config_root.join(name).join("config.toml"))
                .into(),
            log_file: config_dir.join("rewa.log"),
            config_dir,
            thumbnail_dir: cache_dir.join("thumbnails"),
            cache_dir,
            control_endpoint: ControlEndpoint::UnixSocket(runtime_root.join("rewa.sock")),
        }
    }

    #[cfg(target_os = "windows")]
    fn discover_windows() -> Self {
        let profile = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| profile.join("AppData").join("Local"));
        let config_dir = local_app_data.join("Rewa");
        let cache_dir = config_dir.join("Cache");
        adopt_wreath_files(&local_app_data.join("Wreath"), &config_dir);
        Self {
            config_file: config_dir.join("config.toml"),
            favorites_file: config_dir.join("favorites.json"),
            legacy_config_files: ["Wreath", "Trace", "Riftclip"]
                .map(|name| local_app_data.join(name).join("config.toml"))
                .into(),
            log_file: config_dir.join("rewa.log"),
            config_dir,
            thumbnail_dir: cache_dir.join("thumbnails"),
            cache_dir,
            control_endpoint: ControlEndpoint::NamedPipe(r"\\.\pipe\rewa".into()),
        }
    }

    #[allow(dead_code)]
    fn fallback() -> Self {
        let root = std::env::temp_dir().join("rewa");
        Self {
            config_file: root.join("config.toml"),
            favorites_file: root.join("favorites.json"),
            legacy_config_files: Vec::new(),
            thumbnail_dir: root.join("cache").join("thumbnails"),
            cache_dir: root.join("cache"),
            log_file: root.join("rewa.log"),
            config_dir: root,
            control_endpoint: if cfg!(target_os = "windows") {
                ControlEndpoint::NamedPipe(r"\\.\pipe\rewa".into())
            } else {
                ControlEndpoint::UnixSocket(std::env::temp_dir().join("rewa.sock"))
            },
        }
    }

    #[cfg(target_os = "linux")]
    pub fn socket_file(&self) -> &Path {
        match &self.control_endpoint {
            ControlEndpoint::UnixSocket(path) => path,
            ControlEndpoint::NamedPipe(_) => unreachable!("Linux requires a Unix socket"),
        }
    }

    #[cfg(target_os = "windows")]
    pub fn pipe_name(&self) -> &str {
        match &self.control_endpoint {
            ControlEndpoint::NamedPipe(name) => name,
            ControlEndpoint::UnixSocket(_) => unreachable!("Windows requires a named pipe"),
        }
    }
}
