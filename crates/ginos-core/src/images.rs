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
}
