use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreview {
    pub cleaned_text: String,
    pub scalar_count: usize,
    pub utf16_batches: Vec<Vec<u16>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextError {
    Empty,
    TooLong { actual: usize, limit: usize },
    InvalidBatchSize,
}

pub fn clean_text(input: &str) -> String {
    input
        .chars()
        .filter(|character| !is_disallowed(*character))
        .collect()
}

pub fn scalar_count(input: &str) -> usize {
    input.chars().count()
}

pub fn preview_text(
    input: &str,
    character_limit: usize,
    batch_size: usize,
) -> Result<TextPreview, TextError> {
    if batch_size == 0 {
        return Err(TextError::InvalidBatchSize);
    }

    let cleaned_text = clean_text(input);
    let scalar_count = scalar_count(&cleaned_text);
    if scalar_count == 0 {
        return Err(TextError::Empty);
    }
    if scalar_count > character_limit {
        return Err(TextError::TooLong {
            actual: scalar_count,
            limit: character_limit,
        });
    }

    let characters: Vec<char> = cleaned_text.chars().collect();
    let utf16_batches = characters
        .chunks(batch_size)
        .map(|chunk| {
            chunk
                .iter()
                .copied()
                .collect::<String>()
                .encode_utf16()
                .collect()
        })
        .collect();

    Ok(TextPreview {
        cleaned_text,
        scalar_count,
        utf16_batches,
    })
}

fn is_disallowed(character: char) -> bool {
    matches!(character, '\r' | '\n' | '\0' | '\u{2028}' | '\u{2029}')
        || matches!(character as u32, 0x0001..=0x001f | 0x007f..=0x009f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_line_breaks_nul_and_control_characters() {
        let input = "甲\r\n乙\0\u{0007}丙\u{007f}\u{0085}丁\u{2028}戊\u{2029}己";
        assert_eq!(clean_text(input), "甲乙丙丁戊己");
    }

    #[test]
    fn counts_unicode_scalars_not_bytes_or_utf16_units() {
        assert_eq!(scalar_count("中𠮷🙂"), 3);
    }

    #[test]
    fn chunks_by_scalar_without_splitting_surrogate_pairs() {
        let preview = preview_text("甲乙𠮷丙🙂丁", 100, 2).unwrap();
        assert_eq!(preview.scalar_count, 6);
        assert_eq!(preview.utf16_batches.len(), 3);
        assert_eq!(
            String::from_utf16(&preview.utf16_batches[0]).unwrap(),
            "甲乙"
        );
        assert_eq!(
            String::from_utf16(&preview.utf16_batches[1]).unwrap(),
            "𠮷丙"
        );
        assert_eq!(
            String::from_utf16(&preview.utf16_batches[2]).unwrap(),
            "🙂丁"
        );
        assert_eq!(preview.utf16_batches[1].len(), 3);
        assert_eq!(preview.utf16_batches[2].len(), 3);
    }

    #[test]
    fn validates_cleaned_length() {
        assert_eq!(preview_text("\n\0", 10, 5), Err(TextError::Empty));
        assert_eq!(
            preview_text("一二三", 2, 5),
            Err(TextError::TooLong {
                actual: 3,
                limit: 2
            })
        );
    }
}
