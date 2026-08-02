use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_LOG_CHARS: usize = 12_000;
const MAX_LOG_BYTES: u64 = 512 * 1024;
const RETAIN_LOG_BYTES: usize = 384 * 1024;

pub fn append(path: &Path, stage: &str, message: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    trim_if_oversized(path)?;
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "[{timestamp_ms}][{stage}] {message}")?;
    file.flush()
}

fn trim_if_oversized(path: &Path) -> std::io::Result<()> {
    if !path.exists() || fs::metadata(path)?.len() <= MAX_LOG_BYTES {
        return Ok(());
    }

    let content = fs::read_to_string(path)?;
    let mut start = content.len().saturating_sub(RETAIN_LOG_BYTES);
    while start < content.len() && !content.is_char_boundary(start) {
        start += 1;
    }
    fs::write(path, &content[start..])
}

pub fn read(path: &Path) -> std::io::Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    let mut content = String::new();
    fs::File::open(path)?.read_to_string(&mut content)?;
    let char_count = content.chars().count();
    if char_count <= MAX_LOG_CHARS {
        return Ok(content);
    }
    Ok(content.chars().skip(char_count - MAX_LOG_CHARS).collect())
}

pub fn clear(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        fs::write(path, [])?;
    }
    Ok(())
}

pub fn export(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::copy(source, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT_LOG_TEST_ID: AtomicUsize = AtomicUsize::new(0);

    fn test_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "hd2cn-log-test-{}-{}.log",
            std::process::id(),
            NEXT_LOG_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn appends_reads_and_clears_utf8_logs() {
        let path = test_path();
        let _ = clear(&path);
        append(&path, "stage", "中文日志").unwrap();
        assert!(read(&path).unwrap().contains("中文日志"));
        clear(&path).unwrap();
        assert_eq!(read(&path).unwrap(), "");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn compacts_an_oversized_log_before_appending() {
        let path = test_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, "a".repeat((MAX_LOG_BYTES as usize) + 1)).unwrap();
        append(&path, "stage", "tail").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.len() <= RETAIN_LOG_BYTES + 64);
        assert!(content.ends_with("[stage] tail\n"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn exports_the_current_log_to_a_separate_file() {
        let source = test_path();
        let destination = source.with_extension("export.log");
        let _ = clear(&source);
        let _ = clear(&destination);
        append(&source, "diagnostic", "export me").unwrap();

        export(&source, &destination).unwrap();

        assert_eq!(
            fs::read_to_string(&source).unwrap(),
            fs::read_to_string(&destination).unwrap()
        );
        let _ = fs::remove_file(source);
        let _ = fs::remove_file(destination);
    }
}
