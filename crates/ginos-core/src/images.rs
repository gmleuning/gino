//! Raster images: loading by reference, caching, and placement in a frame.
//! Every image is decoded once and kept as PNG bytes, which both renderers
//! accept. SVG images come with a later milestone.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::ImageEncoder;

use crate::error::Error;
use crate::model::{Frame, ImageCrop, ImageFit, Rect};

#[derive(Debug)]
pub struct LoadedImage {
    pub width: u32,
    pub height: u32,
    /// The image re-encoded as PNG (RGBA, 8 bits per channel).
    pub png: Vec<u8>,
}

/// Loads images relative to a document folder and keeps them.
#[derive(Debug, Default)]
pub struct ImageCache {
    folder: PathBuf,
    loaded: HashMap<PathBuf, Arc<LoadedImage>>,
}

impl ImageCache {
    pub fn new(folder: &Path) -> Self {
        Self {
            folder: folder.to_owned(),
            loaded: HashMap::new(),
        }
    }

    /// Resolves a stored image path: relative paths are relative to the
    /// document folder.
    pub fn resolve(&self, stored: &str) -> PathBuf {
        let path = Path::new(stored);
        if path.is_absolute() {
            path.to_owned()
        } else {
            self.folder.join(path)
        }
    }

    pub fn load(&mut self, stored: &str) -> Result<Arc<LoadedImage>, Error> {
        let path = self.resolve(stored);
        if let Some(image) = self.loaded.get(&path) {
            return Ok(image.clone());
        }
        let image = Arc::new(decode(&path)?);
        self.loaded.insert(path, image.clone());
        Ok(image)
    }

    /// Forgets a cached image so the next load reads the file again.
    pub fn forget(&mut self, stored: &str) {
        let path = self.resolve(stored);
        self.loaded.remove(&path);
    }
}

fn decode(path: &Path) -> Result<LoadedImage, Error> {
    let reader = image::ImageReader::open(path)
        .map_err(|error| Error::Image {
            path: path.to_owned(),
            message: error.to_string(),
        })?
        .with_guessed_format()
        .map_err(|error| Error::Image {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    let decoded = reader.decode().map_err(|error| Error::Image {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(Cursor::new(&mut png))
        .write_image(rgba.as_raw(), width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| Error::Image {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    Ok(LoadedImage { width, height, png })
}

/// The crop (offset and scale) that `fit` produces for an image in a frame.
pub fn crop_for_fit(frame: &Frame, image: &LoadedImage, fit: ImageFit) -> ImageCrop {
    let (iw, ih) = (f64::from(image.width.max(1)), f64::from(image.height.max(1)));
    match fit {
        ImageFit::Manual => frame.crop.unwrap_or_else(|| crop_for_fit(frame, image, ImageFit::Fill)),
        ImageFit::Fill | ImageFit::Fit => {
            let scale_x = frame.w / iw;
            let scale_y = frame.h / ih;
            let scale = if fit == ImageFit::Fill {
                scale_x.max(scale_y)
            } else {
                scale_x.min(scale_y)
            };
            ImageCrop {
                offset_x: (frame.w - iw * scale) / 2.0,
                offset_y: (frame.h - ih * scale) / 2.0,
                scale,
            }
        }
    }
}

/// Where the whole image sits on the page, in millimetres, before clipping
/// to the frame.
pub fn placement(frame: &Frame, image: &LoadedImage) -> Rect {
    let crop = crop_for_fit(frame, image, frame.fit);
    Rect::new(
        frame.x + crop.offset_x,
        frame.y + crop.offset_y,
        f64::from(image.width) * crop.scale,
        f64::from(image.height) * crop.scale,
    )
}

/// What a placed image is made of: its pixels, the resolution the file
/// claims, and the resolution it prints at in its frame.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ImageInfo {
    pub path: String,
    pub pixel_width: u32,
    pub pixel_height: u32,
    /// The dots per inch written in the file, when it says (PNG pHYs, JPEG JFIF).
    pub stated_dpi: Option<f64>,
    /// Pixels per inch at the size the image is placed in its frame.
    pub effective_dpi: f64,
    /// The whole picture, uncropped, as a small PNG data URI.
    pub thumbnail: String,
}

/// Pixels per inch of an image at its placement in a frame.
pub fn effective_dpi(frame: &Frame, image: &LoadedImage) -> f64 {
    let crop = crop_for_fit(frame, image, frame.fit);
    if crop.scale <= 0.0 {
        return 0.0;
    }
    // crop.scale is millimetres per pixel.
    25.4 / crop.scale
}

/// The resolution a PNG or JPEG file states, in dots per inch.
pub fn stated_dpi(path: &Path) -> Option<f64> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        // Walk the chunks to pHYs: pixels per unit X and Y, then the unit (1 = metre).
        let mut at = 8;
        while at + 8 <= bytes.len() {
            let length = u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?) as usize;
            let kind = &bytes[at + 4..at + 8];
            let data = bytes.get(at + 8..at + 8 + length)?;
            if kind == b"pHYs" && length >= 9 {
                let x = u32::from_be_bytes(data[0..4].try_into().ok()?);
                return (data[8] == 1 && x > 0).then(|| (f64::from(x) * 0.0254 * 10.0).round() / 10.0);
            }
            if kind == b"IDAT" {
                return None;
            }
            at += 12 + length;
        }
        return None;
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        // The JFIF APP0 segment: units (1 = per inch, 2 = per centimetre), then X density.
        let at = bytes.windows(5).take(64).position(|window| window == b"JFIF\0")?;
        let units = *bytes.get(at + 7)?;
        let x = u16::from_be_bytes([*bytes.get(at + 8)?, *bytes.get(at + 9)?]);
        return match units {
            1 if x > 1 => Some(f64::from(x)),
            2 if x > 1 => Some((f64::from(x) * 2.54 * 10.0).round() / 10.0),
            _ => None,
        };
    }
    None
}

/// A small PNG of the whole image, at most `largest` pixels on its long side.
pub fn thumbnail_png(image: &LoadedImage, largest: u32) -> Result<Vec<u8>, Error> {
    let decoded = image::load_from_memory_with_format(&image.png, image::ImageFormat::Png)
        .map_err(|error| Error::Invalid(format!("the image could not be read again: {error}")))?;
    let small = decoded.thumbnail(largest, largest).to_rgba8();
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(Cursor::new(&mut png))
        .write_image(small.as_raw(), small.width(), small.height(), image::ExtendedColorType::Rgba8)
        .map_err(|error| Error::Invalid(format!("the thumbnail could not be written: {error}")))?;
    Ok(png)
}

/// Resamples every image in a scene that is finer than `dpi` at its placed
/// size down to `dpi`. Images at or below it are left alone; nothing is
/// enlarged. The same image placed twice at the same size is resampled once.
pub fn limit_resolution(scene: &crate::scene::Scene, dpi: f64) -> Result<crate::scene::Scene, Error> {
    use crate::scene::{GroupItem, ImageItem, Item, Page};
    let mut done: HashMap<(usize, u32), Arc<LoadedImage>> = HashMap::new();

    fn limit_items(
        items: &[Item],
        dpi: f64,
        done: &mut HashMap<(usize, u32), Arc<LoadedImage>>,
    ) -> Result<Vec<Item>, Error> {
        items
            .iter()
            .map(|item| {
                Ok(match item {
                    Item::Image(image) => {
                        let target = (image.placement.w / 25.4 * dpi).ceil().max(1.0) as u32;
                        // Two percent of slack so an image already at the target is not resampled.
                        if f64::from(image.image.width) <= f64::from(target) * 1.02 {
                            item.clone()
                        } else {
                            let key = (Arc::as_ptr(&image.image) as usize, target);
                            let smaller = match done.get(&key) {
                                Some(found) => found.clone(),
                                None => {
                                    let made = Arc::new(resample(&image.image, target)?);
                                    done.insert(key, made.clone());
                                    made
                                }
                            };
                            Item::Image(ImageItem {
                                image: smaller,
                                placement: image.placement,
                                clip: image.clip,
                            })
                        }
                    }
                    Item::Group(group) => Item::Group(GroupItem {
                        transform: group.transform,
                        clip: group.clip,
                        items: limit_items(&group.items, dpi, done)?,
                    }),
                    other => other.clone(),
                })
            })
            .collect()
    }

    let pages = scene
        .pages
        .iter()
        .map(|page| {
            let frames = page
                .frames
                .iter()
                .map(|frame| {
                    Ok(crate::scene::FrameScene {
                        id: frame.id.clone(),
                        items: limit_items(&frame.items, dpi, &mut done)?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            Ok(Page {
                width: page.width,
                height: page.height,
                frames,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(crate::scene::Scene { pages })
}

fn resample(image: &LoadedImage, width: u32) -> Result<LoadedImage, Error> {
    let decoded = image::load_from_memory_with_format(&image.png, image::ImageFormat::Png)
        .map_err(|error| Error::Invalid(format!("the image could not be read again: {error}")))?
        .to_rgba8();
    let height = ((f64::from(image.height) * f64::from(width) / f64::from(image.width.max(1))).round() as u32).max(1);
    let resized = image::imageops::resize(&decoded, width, height, image::imageops::FilterType::Lanczos3);
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(Cursor::new(&mut png))
        .write_image(resized.as_raw(), width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| Error::Invalid(format!("the resampled image could not be written: {error}")))?;
    Ok(LoadedImage { width, height, png })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32) -> LoadedImage {
        LoadedImage {
            width,
            height,
            png: Vec::new(),
        }
    }

    #[test]
    fn fill_covers_and_fit_contains() {
        let frame = Frame::image("i", 10.0, 10.0, 40.0, 20.0, Some("x.png"));
        let landscape = image(400, 100);
        // Fill: height decides (20 / 100 = 0.2), width overflows by 40 mm, centred.
        assert_eq!(placement(&frame, &landscape), Rect::new(-10.0, 10.0, 80.0, 20.0));
        let mut fitted = frame.clone();
        fitted.fit = ImageFit::Fit;
        // Fit: width decides (40 / 400 = 0.1), image is 10 mm tall, centred vertically.
        assert_eq!(placement(&fitted, &landscape), Rect::new(10.0, 15.0, 40.0, 10.0));
    }

    #[test]
    fn manual_crop_is_used_as_given() {
        let mut frame = Frame::image("i", 10.0, 10.0, 40.0, 20.0, Some("x.png"));
        frame.fit = ImageFit::Manual;
        frame.crop = Some(ImageCrop {
            offset_x: -5.0,
            offset_y: 2.0,
            scale: 0.05,
        });
        assert_eq!(placement(&frame, &image(400, 100)), Rect::new(5.0, 12.0, 20.0, 5.0));
    }

    #[test]
    fn a_missing_file_is_a_plain_error() {
        let mut cache = ImageCache::new(Path::new("C:\\nowhere"));
        let error = cache.load("missing.png").unwrap_err();
        assert!(error.to_string().starts_with("the image C:\\nowhere\\missing.png could not be loaded"));
    }

    #[test]
    fn effective_dpi_follows_the_placed_size() {
        // 1000 pixels filling a frame 100 mm wide: 254 pixels per inch.
        let frame = Frame::image("picture", 0.0, 0.0, 100.0, 50.0, Some("x.png"));
        let picture = image(1000, 500);
        assert!((effective_dpi(&frame, &picture) - 254.0).abs() < 1e-9);
    }

    #[test]
    fn stated_dpi_from_png_and_jpeg_headers() {
        let folder = std::env::temp_dir().join("ginos-stated-dpi");
        std::fs::create_dir_all(&folder).unwrap();
        // A PNG whose pHYs says 11811 pixels per metre (300 dpi).
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&[0; 13 + 4]);
        png.extend_from_slice(&9u32.to_be_bytes());
        png.extend_from_slice(b"pHYs");
        png.extend_from_slice(&11811u32.to_be_bytes());
        png.extend_from_slice(&11811u32.to_be_bytes());
        png.push(1);
        png.extend_from_slice(&[0; 4]);
        let path = folder.join("a.png");
        std::fs::write(&path, &png).unwrap();
        assert_eq!(stated_dpi(&path), Some(300.0));
        // A JPEG whose JFIF header says 72 dots per inch.
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 1, 1, 0, 72, 0, 72, 0, 0];
        let path = folder.join("b.jpg");
        std::fs::write(&path, jpeg).unwrap();
        assert_eq!(stated_dpi(&path), Some(72.0));
    }
}

/// A picture for the desk behind the pages (a preference, not part of any
/// document): read from `path`, reduced to at most `largest` pixels on its
/// long side and returned as a JPEG data URL the interface can show.
pub fn desk_background_data_url(path: &Path, largest: u32) -> Result<String, Error> {
    use base64::Engine;
    let decoded = image::open(path)
        .map_err(|error| Error::Invalid(format!("{} could not be read as a picture: {error}", path.display())))?;
    let small = if decoded.width().max(decoded.height()) > largest {
        decoded.resize(largest, largest, image::imageops::FilterType::Lanczos3)
    } else {
        decoded
    };
    let rgb = small.to_rgb8();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(Cursor::new(&mut jpeg), 88)
        .write_image(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
        .map_err(|error| Error::Invalid(format!("the picture could not be prepared: {error}")))?;
    Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpeg)))
}
