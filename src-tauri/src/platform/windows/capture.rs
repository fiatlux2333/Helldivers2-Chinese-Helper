use crate::{
    core::{
        session::TargetIdentity,
        translation::{NormalizedRegion, TranslationError},
    },
    platform::windows::target,
};
use base64::Engine;
use image::{DynamicImage, ImageBuffer, ImageFormat, Luma, Rgba, imageops};
use serde::{Deserialize, Serialize};
use std::{ffi::c_void, io::Cursor, mem::size_of};
use windows::{
    Globalization::Language,
    Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Security::Cryptography::CryptographicBuffer,
    Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, ClientToScreen,
            CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject,
            GetDC, GetDIBits, HGDIOBJ, ReleaseDC, SRCCOPY, SelectObject,
        },
        System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
        UI::WindowsAndMessaging::GetClientRect,
    },
    core::HSTRING,
};

struct WinRtApartment;

impl WinRtApartment {
    fn initialize() -> Result<Self, TranslationError> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|error| {
            TranslationError::Response(format!("Windows OCR 初始化失败：{error}"))
        })?;
        Ok(Self)
    }
}

impl Drop for WinRtApartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

#[derive(Debug, Clone)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLanguage {
    pub tag: String,
    pub display_name: String,
    pub native_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationPreview {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PositionedOcrLine {
    pub text: String,
    pub top: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrReading {
    pub language: String,
    pub lines: Vec<PositionedOcrLine>,
}

#[derive(Debug, Clone, PartialEq)]
struct PositionedOcrWord {
    text: String,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

#[derive(Debug)]
struct OcrWordRow {
    words: Vec<PositionedOcrWord>,
    top: f32,
    bottom: f32,
}

pub fn list_ocr_languages() -> Result<Vec<OcrLanguage>, TranslationError> {
    let _apartment = WinRtApartment::initialize()?;
    let languages = OcrEngine::AvailableRecognizerLanguages()
        .map_err(|error| TranslationError::Response(error.to_string()))?;
    let mut output = Vec::with_capacity(languages.Size().unwrap_or(0) as usize);
    for index in 0..languages.Size().unwrap_or(0) {
        let language = languages
            .GetAt(index)
            .map_err(|error| TranslationError::Response(error.to_string()))?;
        output.push(OcrLanguage {
            tag: language
                .LanguageTag()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            display_name: language
                .DisplayName()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            native_name: language
                .NativeName()
                .map(|value| value.to_string())
                .unwrap_or_default(),
        });
    }
    output.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    Ok(output)
}

pub fn preferred_english_ocr_language(languages: &[OcrLanguage]) -> Option<String> {
    languages
        .iter()
        .find(|language| language.tag.eq_ignore_ascii_case("en-US"))
        .or_else(|| {
            languages
                .iter()
                .find(|language| language.tag.to_ascii_lowercase().starts_with("en-"))
        })
        .map(|language| language.tag.clone())
}

pub fn preferred_chinese_ocr_language(languages: &[OcrLanguage]) -> Option<String> {
    ["zh-Hans-CN", "zh-CN", "zh-Hans"]
        .iter()
        .find_map(|preferred| {
            languages
                .iter()
                .find(|language| language.tag.eq_ignore_ascii_case(preferred))
        })
        .or_else(|| {
            languages.iter().find(|language| {
                let tag = language.tag.to_ascii_lowercase();
                tag.starts_with("zh-hans") || tag.starts_with("zh-cn")
            })
        })
        .or_else(|| {
            languages
                .iter()
                .find(|language| language.tag.to_ascii_lowercase().starts_with("zh-"))
        })
        .map(|language| language.tag.clone())
}

pub fn capture_client(
    identity: &TargetIdentity,
    title_keyword: &str,
) -> Result<CapturedImage, TranslationError> {
    if target::validate_foreground(identity, title_keyword) != Ok(true) {
        return Err(TranslationError::Response(
            "目标游戏窗口必须位于前台".to_owned(),
        ));
    }
    let hwnd = HWND(identity.hwnd as usize as *mut c_void);
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect) }
        .map_err(|error| TranslationError::Response(error.to_string()))?;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Err(TranslationError::Response("游戏客户区尺寸无效".to_owned()));
    }

    let mut client_origin = POINT::default();
    if !unsafe { ClientToScreen(hwnd, &mut client_origin) }.as_bool() {
        return Err(TranslationError::Response(
            "无法计算游戏客户区的屏幕坐标".to_owned(),
        ));
    }

    // HD2 uses a hardware-accelerated Stingray surface. Reading its window DC can
    // return a valid but black bitmap, so capture the verified foreground pixels
    // from the desktop at the client area's screen coordinates instead.
    let source_dc = unsafe { GetDC(None) };
    if source_dc.is_invalid() {
        return Err(TranslationError::Response(
            "无法获取游戏窗口画面".to_owned(),
        ));
    }
    let memory_dc = unsafe { CreateCompatibleDC(Some(source_dc)) };
    if memory_dc.is_invalid() {
        unsafe { ReleaseDC(None, source_dc) };
        return Err(TranslationError::Response("无法创建截图缓冲区".to_owned()));
    }
    let bitmap = unsafe { CreateCompatibleBitmap(source_dc, width, height) };
    if bitmap.is_invalid() {
        unsafe {
            let _ = DeleteDC(memory_dc);
            ReleaseDC(None, source_dc);
        }
        return Err(TranslationError::Response("无法创建截图位图".to_owned()));
    }
    let previous = unsafe { SelectObject(memory_dc, HGDIOBJ(bitmap.0)) };
    let copied = unsafe {
        BitBlt(
            memory_dc,
            0,
            0,
            width,
            height,
            Some(source_dc),
            client_origin.x,
            client_origin.y,
            SRCCOPY | CAPTUREBLT,
        )
    };
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    let scanlines = if copied.is_ok() {
        unsafe {
            GetDIBits(
                memory_dc,
                bitmap,
                0,
                height as u32,
                Some(pixels.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            )
        }
    } else {
        0
    };
    unsafe {
        let _ = SelectObject(memory_dc, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        ReleaseDC(None, source_dc);
    }
    if scanlines != height {
        return Err(TranslationError::Response("游戏画面截图失败".to_owned()));
    }
    #[cfg(debug_assertions)]
    {
        let sampled = pixels.chunks_exact(4).step_by(2048);
        let (mut minimum, mut maximum, mut count) = (u8::MAX, u8::MIN, 0usize);
        for pixel in sampled {
            let brightness = pixel[0].max(pixel[1]).max(pixel[2]);
            minimum = minimum.min(brightness);
            maximum = maximum.max(brightness);
            count += 1;
        }
        eprintln!(
            "[hd2cn][capture] stage=client_pixels hwnd=0x{:X} origin=({}, {}) size={}x{} scanlines={} samples={} brightness_min={} brightness_max={}",
            identity.hwnd,
            client_origin.x,
            client_origin.y,
            width,
            height,
            scanlines,
            count,
            minimum,
            maximum
        );
    }
    Ok(CapturedImage {
        width: width as u32,
        height: height as u32,
        bgra: pixels,
    })
}

impl CapturedImage {
    pub fn crop(&self, region: NormalizedRegion) -> Result<Self, TranslationError> {
        let region = region.validate()?;
        let x = (region.x * self.width as f64).round() as u32;
        let y = (region.y * self.height as f64).round() as u32;
        let width = (region.width * self.width as f64).round().max(1.0) as u32;
        let height = (region.height * self.height as f64).round().max(1.0) as u32;
        if x >= self.width || y >= self.height || x + width > self.width || y + height > self.height
        {
            return Err(TranslationError::InvalidRegion);
        }
        let mut bgra = vec![0u8; width as usize * height as usize * 4];
        let source_stride = self.width as usize * 4;
        let target_stride = width as usize * 4;
        for row in 0..height as usize {
            let source_start = (y as usize + row) * source_stride + x as usize * 4;
            let target_start = row * target_stride;
            bgra[target_start..target_start + target_stride]
                .copy_from_slice(&self.bgra[source_start..source_start + target_stride]);
        }
        Ok(Self {
            width,
            height,
            bgra,
        })
    }

    pub fn calibration_preview(&self) -> Result<CalibrationPreview, TranslationError> {
        let rgba = bgra_to_rgba(&self.bgra);
        let image = ImageBuffer::<Rgba<u8>, _>::from_raw(self.width, self.height, rgba)
            .ok_or_else(|| TranslationError::Response("截图像素数据无效".to_owned()))?;
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, ImageFormat::Png)
            .map_err(|error| TranslationError::Response(error.to_string()))?;
        Ok(CalibrationPreview {
            data_url: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(encoded.into_inner())
            ),
            width: self.width,
            height: self.height,
        })
    }

    pub fn recognize(&self, language_tag: &str) -> Result<OcrReading, TranslationError> {
        let reading = self.recognize_allow_empty(language_tag)?;
        if reading.lines.is_empty() {
            return Err(TranslationError::Response(
                "聊天区域未识别到文字".to_owned(),
            ));
        }
        Ok(reading)
    }

    pub fn recognize_allow_empty(
        &self,
        language_tag: &str,
    ) -> Result<OcrReading, TranslationError> {
        let _apartment = WinRtApartment::initialize()?;
        let (width, height, bgra) = preprocess_for_ocr(self)?;
        let buffer = CryptographicBuffer::CreateFromByteArray(&bgra)
            .map_err(|error| TranslationError::Response(error.to_string()))?;
        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            width as i32,
            height as i32,
        )
        .map_err(|error| TranslationError::Response(error.to_string()))?;
        let engine = if language_tag.trim().is_empty() || language_tag == "auto" {
            OcrEngine::TryCreateFromUserProfileLanguages()
        } else {
            let language = Language::CreateLanguage(&HSTRING::from(language_tag))
                .map_err(|error| TranslationError::Response(error.to_string()))?;
            OcrEngine::TryCreateFromLanguage(&language)
        }
        .map_err(|error| {
            TranslationError::Response(format!("OCR 语言 {language_tag} 不可用：{error}"))
        })?;
        let actual_language = engine
            .RecognizerLanguage()
            .and_then(|language| language.LanguageTag())
            .map(|value| value.to_string())
            .unwrap_or_else(|_| language_tag.to_owned());
        let result = engine
            .RecognizeAsync(&bitmap)
            .map_err(|error| TranslationError::Response(error.to_string()))?
            .join()
            .map_err(|error| TranslationError::Response(error.to_string()))?;
        let lines = positioned_ocr_lines(&result)?;
        Ok(OcrReading {
            language: actual_language,
            lines,
        })
    }
}

fn preprocess_for_ocr(image: &CapturedImage) -> Result<(u32, u32, Vec<u8>), TranslationError> {
    let rgba = bgra_to_rgba(&image.bgra);
    let source = ImageBuffer::<Rgba<u8>, _>::from_raw(image.width, image.height, rgba)
        .ok_or_else(|| TranslationError::Response("截图像素数据无效".to_owned()))?;
    let max_dimension = OcrEngine::MaxImageDimension().unwrap_or(2600).max(1);
    let largest = image.width.max(image.height).max(1);
    let factor = (max_dimension as f32 / largest as f32).clamp(1.0, 2.0);
    let width = (image.width as f32 * factor).round() as u32;
    let height = (image.height as f32 * factor).round() as u32;
    let resized = imageops::resize(&source, width, height, imageops::FilterType::CatmullRom);
    let grayscale = imageops::grayscale(&resized);
    let contrasted = imageops::contrast(&grayscale, 35.0);
    let mut bgra = Vec::with_capacity(width as usize * height as usize * 4);
    for Luma([value]) in contrasted.pixels() {
        bgra.extend_from_slice(&[*value, *value, *value, 255]);
    }
    Ok((width, height, bgra))
}

fn bgra_to_rgba(source: &[u8]) -> Vec<u8> {
    source
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
        .collect()
}

fn normalize_ocr_text(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn positioned_ocr_lines(
    result: &windows::Media::Ocr::OcrResult,
) -> Result<Vec<PositionedOcrLine>, TranslationError> {
    let lines = result
        .Lines()
        .map_err(|error| TranslationError::Response(error.to_string()))?;
    let mut output = Vec::with_capacity(lines.Size().unwrap_or(0) as usize);
    for line in lines {
        let text = normalize_ocr_text(
            &line
                .Text()
                .map(|value| value.to_string())
                .unwrap_or_default(),
        );
        if text.is_empty() {
            continue;
        }
        let words = line
            .Words()
            .map_err(|error| TranslationError::Response(error.to_string()))?;
        let mut positioned_words = Vec::with_capacity(words.Size().unwrap_or(0) as usize);
        for word in words {
            let bounds = word
                .BoundingRect()
                .map_err(|error| TranslationError::Response(error.to_string()))?;
            let word_text = word
                .Text()
                .map(|value| value.to_string())
                .unwrap_or_default();
            if !word_text.trim().is_empty() {
                positioned_words.push(PositionedOcrWord {
                    text: word_text,
                    left: bounds.X,
                    top: bounds.Y,
                    width: bounds.Width,
                    height: bounds.Height,
                });
            }
        }
        let mut visual_lines = cluster_ocr_words_by_visual_row(positioned_words);
        if visual_lines.len() == 1 {
            visual_lines[0].text = text;
        }
        output.extend(visual_lines);
    }
    output.sort_by(|left, right| left.top.total_cmp(&right.top));
    Ok(output)
}

fn cluster_ocr_words_by_visual_row(
    mut words: Vec<PositionedOcrWord>,
) -> Vec<PositionedOcrLine> {
    words.retain(|word| {
        word.left.is_finite()
            && word.top.is_finite()
            && word.width.is_finite()
            && word.height.is_finite()
            && word.width > 0.0
            && word.height > 0.0
            && !word.text.trim().is_empty()
    });
    words.sort_by(|left, right| {
        word_center_y(left)
            .total_cmp(&word_center_y(right))
            .then_with(|| left.left.total_cmp(&right.left))
    });

    let mut rows: Vec<OcrWordRow> = Vec::new();
    for word in words {
        let center = word_center_y(&word);
        let best_row = rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let row_height = row.bottom - row.top;
                let row_center = row.top + row_height / 2.0;
                let center_distance = (center - row_center).abs();
                let overlap = (word.top + word.height).min(row.bottom) - word.top.max(row.top);
                let overlaps_row = overlap > word.height.min(row_height) * 0.35;
                let centers_align = center_distance <= word.height.max(row_height) * 0.45;
                (overlaps_row || centers_align).then_some((index, center_distance))
            })
            .min_by(|(_, left), (_, right)| left.total_cmp(right))
            .map(|(index, _)| index);

        if let Some(index) = best_row {
            let row = &mut rows[index];
            row.top = row.top.min(word.top);
            row.bottom = row.bottom.max(word.top + word.height);
            row.words.push(word);
        } else {
            let top = word.top;
            let bottom = word.top + word.height;
            rows.push(OcrWordRow {
                words: vec![word],
                top,
                bottom,
            });
        }
    }

    rows.sort_by(|left, right| left.top.total_cmp(&right.top));
    rows.into_iter()
        .filter_map(|mut row| {
            row.words
                .sort_by(|left, right| left.left.total_cmp(&right.left));
            let text = join_ocr_row_words(&row.words);
            (!text.is_empty()).then_some(PositionedOcrLine {
                text,
                top: row.top,
                height: row.bottom - row.top,
            })
        })
        .collect()
}

fn word_center_y(word: &PositionedOcrWord) -> f32 {
    word.top + word.height / 2.0
}

fn join_ocr_row_words(words: &[PositionedOcrWord]) -> String {
    let mut output = String::new();
    let mut previous: Option<&PositionedOcrWord> = None;
    for word in words {
        let text = word.text.trim();
        if text.is_empty() {
            continue;
        }
        if let Some(left) = previous {
            let gap = word.left - (left.left + left.width);
            let space_threshold = left.height.min(word.height) * 0.15;
            let previous_character = output.chars().next_back();
            let next_character = text.chars().next();
            let punctuation_boundary = next_character.is_some_and(is_closing_punctuation)
                || previous_character.is_some_and(is_opening_punctuation);
            let adjacent_han = previous_character.is_some_and(is_han_character)
                && next_character.is_some_and(is_han_character);
            if gap > space_threshold.max(1.5) && !punctuation_boundary && !adjacent_han {
                output.push(' ');
            }
        }
        output.push_str(text);
        previous = Some(word);
    }
    output
}

fn is_han_character(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
    )
}

fn is_closing_punctuation(character: char) -> bool {
    matches!(
        character,
        ',' | '.' | ':' | ';' | '!' | '?' | ')' | ']' | '}' | '，' | '。' | '：' | '；'
            | '！' | '？' | '）' | '】' | '》'
    )
}

fn is_opening_punctuation(character: char) -> bool {
    matches!(character, '(' | '[' | '{' | '（' | '【' | '《')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crops_using_normalized_coordinates() {
        let image = CapturedImage {
            width: 4,
            height: 2,
            bgra: (0u8..32).collect(),
        };
        let crop = image
            .crop(NormalizedRegion {
                x: 0.5,
                y: 0.0,
                width: 0.5,
                height: 1.0,
            })
            .unwrap();
        assert_eq!((crop.width, crop.height), (2, 2));
        assert_eq!(
            crop.bgra,
            vec![8, 9, 10, 11, 12, 13, 14, 15, 24, 25, 26, 27, 28, 29, 30, 31]
        );
    }

    #[test]
    fn normalizes_ocr_lines() {
        assert_eq!(
            normalize_ocr_text("  hello  \r\n\r\n world "),
            "hello\nworld"
        );
    }

    #[test]
    fn selects_an_installed_english_ocr_language() {
        let languages = vec![
            OcrLanguage {
                tag: "zh-Hans-CN".to_owned(),
                display_name: "Chinese".to_owned(),
                native_name: "简体中文".to_owned(),
            },
            OcrLanguage {
                tag: "en-GB".to_owned(),
                display_name: "English (United Kingdom)".to_owned(),
                native_name: "English (United Kingdom)".to_owned(),
            },
        ];
        assert_eq!(
            preferred_english_ocr_language(&languages).as_deref(),
            Some("en-GB")
        );
        assert_eq!(preferred_english_ocr_language(&languages[..1]), None);
    }

    #[test]
    fn selects_an_installed_simplified_chinese_ocr_language() {
        let languages = vec![
            OcrLanguage {
                tag: "zh-Hant-TW".to_owned(),
                display_name: "Chinese (Traditional)".to_owned(),
                native_name: "繁體中文".to_owned(),
            },
            OcrLanguage {
                tag: "zh-Hans-CN".to_owned(),
                display_name: "Chinese (Simplified)".to_owned(),
                native_name: "简体中文".to_owned(),
            },
        ];
        assert_eq!(
            preferred_chinese_ocr_language(&languages).as_deref(),
            Some("zh-Hans-CN")
        );
    }
}
