use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppConfig {
    pub title_keyword: String,
    pub batch_size: usize,
    pub batch_delay_ms: u64,
    pub character_limit: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title_keyword: "HELLDIVERS".to_owned(),
            batch_size: 5,
            batch_delay_ms: 50,
            character_limit: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigViolation {
    pub field: &'static str,
    pub message: String,
}

impl AppConfig {
    pub fn validate(&self) -> Result<(), Vec<ConfigViolation>> {
        let mut violations = Vec::new();

        if self.title_keyword.trim().is_empty() {
            violations.push(ConfigViolation {
                field: "titleKeyword",
                message: "窗口标题关键词不能为空".to_owned(),
            });
        }
        if !(1..=20).contains(&self.batch_size) {
            violations.push(ConfigViolation {
                field: "batchSize",
                message: "每批字符数必须在 1 到 20 之间".to_owned(),
            });
        }
        if !(10..=500).contains(&self.batch_delay_ms) {
            violations.push(ConfigViolation {
                field: "batchDelayMs",
                message: "批间延迟必须在 10 到 500 毫秒之间".to_owned(),
            });
        }
        if !(1..=500).contains(&self.character_limit) {
            violations.push(ConfigViolation {
                field: "characterLimit",
                message: "字符上限必须在 1 到 500 之间".to_owned(),
            });
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert_eq!(AppConfig::default().validate(), Ok(()));
    }

    #[test]
    fn rejects_all_out_of_range_values() {
        let config = AppConfig {
            title_keyword: "  ".to_owned(),
            batch_size: 0,
            batch_delay_ms: 501,
            character_limit: 501,
        };

        let errors = config.validate().unwrap_err();
        assert_eq!(errors.len(), 4);
        assert!(errors.iter().any(|error| error.field == "titleKeyword"));
        assert!(errors.iter().any(|error| error.field == "batchSize"));
        assert!(errors.iter().any(|error| error.field == "batchDelayMs"));
        assert!(errors.iter().any(|error| error.field == "characterLimit"));
    }

    #[test]
    fn accepts_boundary_values() {
        for config in [
            AppConfig {
                batch_size: 1,
                batch_delay_ms: 10,
                character_limit: 1,
                ..AppConfig::default()
            },
            AppConfig {
                batch_size: 20,
                batch_delay_ms: 500,
                character_limit: 500,
                ..AppConfig::default()
            },
        ] {
            assert_eq!(config.validate(), Ok(()));
        }
    }
}
