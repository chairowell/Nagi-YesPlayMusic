//! Best-effort Nerd Font installation detection.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Status {
    Detected,
    Missing,
    Unknown,
}

pub(crate) fn detect() -> Status {
    platform::detect()
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows", test))]
fn mentions_nerd_font(value: &str) -> bool {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>()
        .contains("nerdfont")
}

#[cfg(target_os = "macos")]
mod platform {
    use std::fs;
    use std::io::ErrorKind;
    use std::path::Path;

    use super::{mentions_nerd_font, Status};

    pub(super) fn detect() -> Status {
        let mut readable_source = false;
        let mut paths = vec![Path::new("/Library/Fonts").to_path_buf()];
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join("Library/Fonts"));
        }

        for path in paths {
            match scan_directory(&path) {
                Ok(true) => return Status::Detected,
                Ok(false) => readable_source = true,
                Err(()) => {}
            }
        }

        if readable_source {
            Status::Missing
        } else {
            Status::Unknown
        }
    }

    fn scan_directory(path: &Path) -> Result<bool, ()> {
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(_) => return Err(()),
        };
        for entry in entries {
            let entry = entry.map_err(|_| ())?;
            if mentions_nerd_font(&entry.file_name().to_string_lossy()) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::process::Command;

    use super::{mentions_nerd_font, Status};

    pub(super) fn detect() -> Status {
        let Ok(output) = Command::new("fc-list").args([":", "family"]).output() else {
            return Status::Unknown;
        };
        if !output.status.success() {
            return Status::Unknown;
        }
        if mentions_nerd_font(&String::from_utf8_lossy(&output.stdout)) {
            Status::Detected
        } else {
            Status::Missing
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::fs;
    use std::io::ErrorKind;
    use std::path::Path;
    use std::process::Command;

    use super::{mentions_nerd_font, Status};

    const FONT_REGISTRY_KEYS: [&str; 2] = [
        r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts",
        r"HKCU\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts",
    ];

    pub(super) fn detect() -> Status {
        let mut readable_source = false;

        for key in FONT_REGISTRY_KEYS {
            match scan_registry(key) {
                Ok(true) => return Status::Detected,
                Ok(false) => readable_source = true,
                Err(()) => {}
            }
        }

        for path in font_directories() {
            match scan_directory(&path) {
                Ok(true) => return Status::Detected,
                Ok(false) => readable_source = true,
                Err(()) => {}
            }
        }

        if readable_source {
            Status::Missing
        } else {
            Status::Unknown
        }
    }

    fn scan_registry(key: &str) -> Result<bool, ()> {
        let output = Command::new("reg.exe")
            .args(["query", key])
            .output()
            .map_err(|_| ())?;
        if !output.status.success() {
            return Err(());
        }
        Ok(mentions_nerd_font(&String::from_utf8_lossy(&output.stdout)))
    }

    fn font_directories() -> Vec<std::path::PathBuf> {
        let mut paths = Vec::with_capacity(2);
        if let Some(windows) = std::env::var_os("WINDIR") {
            paths.push(Path::new(&windows).join("Fonts"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            paths.push(Path::new(&local).join("Microsoft/Windows/Fonts"));
        }
        paths
    }

    fn scan_directory(path: &Path) -> Result<bool, ()> {
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(_) => return Err(()),
        };
        for entry in entries {
            let entry = entry.map_err(|_| ())?;
            if mentions_nerd_font(&entry.file_name().to_string_lossy()) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod platform {
    use super::Status;

    pub(super) const fn detect() -> Status {
        Status::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_spaced_and_compact_nerd_font_names() {
        assert!(mentions_nerd_font("Symbols Nerd Font Mono"));
        assert!(mentions_nerd_font("MesloLGSDZNerdFont-Regular.ttf"));
        assert!(mentions_nerd_font("Caskaydia-Nerd_Font.otf"));
    }

    #[test]
    fn rejects_unrelated_font_names() {
        assert!(!mentions_nerd_font("SF Mono Regular.ttf"));
        assert!(!mentions_nerd_font("Font Awesome 6 Free"));
        assert!(!mentions_nerd_font("Nerdy Sans Font"));
    }
}
