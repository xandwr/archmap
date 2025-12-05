//! Centralized filesystem operations for better testability.
//!
//! This module provides a `FileSystem` trait that abstracts file operations,
//! allowing for easy mocking in tests and consistent error handling.

use std::io;
use std::path::Path;

/// Trait for filesystem operations, enabling dependency injection and testing.
pub trait FileSystem: Send + Sync {
    /// Read the entire contents of a file as a string.
    fn read_to_string(&self, path: &Path) -> io::Result<String>;

    /// Write content to a file, creating it if it doesn't exist.
    fn write(&self, path: &Path, content: &str) -> io::Result<()>;

    /// Check if a path exists.
    fn exists(&self, path: &Path) -> bool;
}

/// Real filesystem implementation using std::fs.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFs;

impl RealFs {
    pub fn new() -> Self {
        Self
    }
}

impl FileSystem for RealFs {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn write(&self, path: &Path, content: &str) -> io::Result<()> {
        std::fs::write(path, content)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

/// Global default filesystem for use when dependency injection isn't practical.
/// This provides a migration path - code can start using `default_fs()` and
/// later be refactored to accept `&dyn FileSystem` parameters.
pub fn default_fs() -> &'static RealFs {
    static INSTANCE: RealFs = RealFs;
    &INSTANCE
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::HashMap;
    use std::sync::RwLock;

    /// In-memory filesystem for testing.
    #[derive(Debug, Default)]
    pub struct MockFs {
        files: RwLock<HashMap<String, String>>,
    }

    impl MockFs {
        pub fn new() -> Self {
            Self {
                files: RwLock::new(HashMap::new()),
            }
        }

        /// Pre-populate the mock filesystem with files.
        pub fn with_files<I, P, C>(files: I) -> Self
        where
            I: IntoIterator<Item = (P, C)>,
            P: AsRef<Path>,
            C: Into<String>,
        {
            let map: HashMap<String, String> = files
                .into_iter()
                .map(|(p, c)| (p.as_ref().to_string_lossy().to_string(), c.into()))
                .collect();
            Self {
                files: RwLock::new(map),
            }
        }

        /// Get all files currently in the mock filesystem.
        pub fn files(&self) -> HashMap<String, String> {
            self.files.read().unwrap().clone()
        }
    }

    impl FileSystem for MockFs {
        fn read_to_string(&self, path: &Path) -> io::Result<String> {
            let key = path.to_string_lossy().to_string();
            self.files
                .read()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, format!("file not found: {}", key))
                })
        }

        fn write(&self, path: &Path, content: &str) -> io::Result<()> {
            let key = path.to_string_lossy().to_string();
            self.files.write().unwrap().insert(key, content.to_string());
            Ok(())
        }

        fn exists(&self, path: &Path) -> bool {
            let key = path.to_string_lossy().to_string();
            self.files.read().unwrap().contains_key(&key)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_mock_fs_read_write() {
            let fs = MockFs::new();
            let path = Path::new("/test/file.txt");

            // File doesn't exist initially
            assert!(!fs.exists(path));
            assert!(fs.read_to_string(path).is_err());

            // Write and read back
            fs.write(path, "hello world").unwrap();
            assert!(fs.exists(path));
            assert_eq!(fs.read_to_string(path).unwrap(), "hello world");
        }

        #[test]
        fn test_mock_fs_with_files() {
            let fs = MockFs::with_files([
                (Path::new("/a.txt"), "content a"),
                (Path::new("/b.txt"), "content b"),
            ]);

            assert_eq!(fs.read_to_string(Path::new("/a.txt")).unwrap(), "content a");
            assert_eq!(fs.read_to_string(Path::new("/b.txt")).unwrap(), "content b");
        }
    }
}
