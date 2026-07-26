use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_LOG_CHARS: usize = 12_000;

pub fn append(path: &Path, stage: &str, message: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "[{timestamp_ms}][{stage}] {message}")?;
    file.flush()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_path() -> PathBuf {
        std::env::temp_dir().join(format!("hd2cn-log-test-{}.log", std::process::id()))
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
}
