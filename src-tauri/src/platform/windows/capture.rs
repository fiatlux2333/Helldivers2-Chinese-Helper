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
        let mut top = f32::MAX;
        let mut bottom = f32::MIN;
        for word in words {
            let bounds = word
                .BoundingRect()
                .map_err(|error| TranslationError::Response(error.to_string()))?;
            top = top.min(bounds.Y);
            bottom = bottom.max(bounds.Y + bounds.Height);
        }
        if top.is_finite() && bottom.is_finite() && bottom > top {
            output.push(PositionedOcrLine {
                text,
                top,
                height: bottom - top,
            });
        }
    }
    output.sort_by(|left, right| left.top.total_cmp(&right.top));
    Ok(output)
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
