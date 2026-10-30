//! Log de consola rotativo por instancia: `console.log`, `console.1.log` … `console.N.log`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct RotatingLog {
    dir: PathBuf,
    max_bytes: u64,
    keep: u32,
    file: File,
    written: u64,
}

impl RotatingLog {
    pub fn open(dir: &Path, max_bytes: u64, keep: u32) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("console.log");
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let written = file.metadata()?.len();
        Ok(Self {
            dir: dir.to_path_buf(),
            max_bytes,
            keep: keep.max(1),
            file,
            written,
        })
    }

    pub fn current_path(&self) -> PathBuf {
        self.dir.join("console.log")
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        self.file.flush()?;
        let oldest = self.dir.join(format!("console.{}.log", self.keep));
        let _ = std::fs::remove_file(&oldest);
        for i in (1..self.keep).rev() {
            let from = self.dir.join(format!("console.{i}.log"));
            if from.exists() {
                std::fs::rename(&from, self.dir.join(format!("console.{}.log", i + 1)))?;
            }
        }
        std::fs::rename(self.current_path(), self.dir.join("console.1.log"))?;
        self.file = OpenOptions::new().create(true).append(true).open(self.current_path())?;
        self.written = 0;
        Ok(())
    }

    pub fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        let bytes = line.len() as u64 + 1;
        if self.written > 0 && self.written + bytes > self.max_bytes {
            self.rotate()?;
        }
        self.file.write_all(line.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.written += bytes;
        Ok(())
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_and_keeps_n() {
        let d = tempfile::tempdir().unwrap();
        let mut log = RotatingLog::open(d.path(), 100, 2).unwrap();
        for i in 0..50 {
            log.write_line(&format!("linea número {i:03}")).unwrap();
        }
        log.flush().unwrap();
        assert!(d.path().join("console.log").exists());
        assert!(d.path().join("console.1.log").exists());
        assert!(d.path().join("console.2.log").exists());
        assert!(!d.path().join("console.3.log").exists());
        for f in ["console.log", "console.1.log", "console.2.log"] {
            assert!(std::fs::metadata(d.path().join(f)).unwrap().len() <= 100);
        }
        let last = std::fs::read_to_string(d.path().join("console.log")).unwrap();
        assert!(last.contains("linea número 049"));
    }
}
