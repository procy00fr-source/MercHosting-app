//! Icono del servidor: cualquier imagen (PNG, JPG, GIF, WebP, BMP, ICO, TIFF) se convierte a
//! un PNG de 64×64, que es lo único que Minecraft acepta como `server-icon.png`.

use crate::AdminError;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

pub const ICON_FILE: &str = "server-icon.png";
pub const ICON_SIZE: u32 = 64;
/// Tamaño máximo del archivo de origen.
pub const MAX_SOURCE_BYTES: u64 = 25 * 1024 * 1024;
/// Lados más grandes se rechazan (evita imágenes gigantes que llenan la memoria).
const MAX_SIDE: u32 = 12_000;

/// Cómo pasar una imagen no cuadrada a un cuadrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IconFit {
    /// Recorta el centro para llenar todo el cuadrado.
    #[default]
    Crop,
    /// La imagen entera, con bordes transparentes.
    Contain,
}

/// Resultado de convertir: el PNG final y datos del original.
#[derive(Debug, Clone)]
pub struct Converted {
    pub png: Vec<u8>,
    pub source_width: u32,
    pub source_height: u32,
    pub format: &'static str,
}

fn format_name(f: ImageFormat) -> &'static str {
    match f {
        ImageFormat::Png => "PNG",
        ImageFormat::Jpeg => "JPG",
        ImageFormat::Gif => "GIF",
        ImageFormat::WebP => "WebP",
        ImageFormat::Bmp => "BMP",
        ImageFormat::Ico => "ICO",
        ImageFormat::Tiff => "TIFF",
        _ => "imagen",
    }
}

/// Convierte los bytes de una imagen en el icono de 64×64 (PNG con transparencia).
pub fn convert(bytes: &[u8], fit: IconFit) -> Result<Converted, AdminError> {
    let bad = || AdminError::Invalid("No se reconoce la imagen. Usa PNG, JPG, GIF, WebP, BMP, ICO o TIFF.".into());
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| bad())?;
    let format = reader.format().ok_or_else(bad)?;
    let (w, h) = reader.into_dimensions().map_err(|_| bad())?;
    if w == 0 || h == 0 {
        return Err(bad());
    }
    if w > MAX_SIDE || h > MAX_SIDE {
        return Err(AdminError::Invalid(format!(
            "La imagen es demasiado grande ({w}×{h}). El máximo es {MAX_SIDE} píxeles por lado."
        )));
    }
    let img = image::load_from_memory_with_format(bytes, format).map_err(|_| bad())?;
    let out = square(&img, fit);
    let mut png = Vec::new();
    DynamicImage::ImageRgba8(out)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|e| AdminError::Io(e.to_string()))?;
    Ok(Converted {
        png,
        source_width: w,
        source_height: h,
        format: format_name(format),
    })
}

fn square(img: &DynamicImage, fit: IconFit) -> RgbaImage {
    let (w, h) = img.dimensions();
    // Las imágenes pequeñas (pixel art) se agrandan sin difuminar.
    let filter = |from: u32| {
        if from < ICON_SIZE {
            FilterType::Nearest
        } else {
            FilterType::Lanczos3
        }
    };
    match fit {
        IconFit::Crop => {
            let side = w.min(h);
            let cropped = img.crop_imm((w - side) / 2, (h - side) / 2, side, side);
            cropped.resize_exact(ICON_SIZE, ICON_SIZE, filter(side)).to_rgba8()
        }
        IconFit::Contain => {
            let side = w.max(h);
            let (nw, nh) = (
                ((w as u64 * ICON_SIZE as u64) / side as u64).max(1) as u32,
                ((h as u64 * ICON_SIZE as u64) / side as u64).max(1) as u32,
            );
            let small = img.resize_exact(nw, nh, filter(side)).to_rgba8();
            let mut canvas = RgbaImage::from_pixel(ICON_SIZE, ICON_SIZE, Rgba([0, 0, 0, 0]));
            image::imageops::overlay(
                &mut canvas,
                &small,
                ((ICON_SIZE - nw) / 2) as i64,
                ((ICON_SIZE - nh) / 2) as i64,
            );
            canvas
        }
    }
}

/// Lee el icono actual y dice si Minecraft lo aceptará (PNG de 64×64).
pub fn inspect(bytes: &[u8]) -> Result<(u32, u32, bool), AdminError> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| AdminError::Invalid(e.to_string()))?;
    let is_png = reader.format() == Some(ImageFormat::Png);
    let (w, h) = reader
        .into_dimensions()
        .map_err(|_| AdminError::Invalid("server-icon.png no es una imagen válida".into()))?;
    Ok((w, h, is_png && w == ICON_SIZE && h == ICON_SIZE))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(img: RgbaImage, f: ImageFormat) -> Vec<u8> {
        let mut v = Vec::new();
        let img = if f == ImageFormat::Jpeg {
            DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(img).to_rgb8())
        } else {
            DynamicImage::ImageRgba8(img)
        };
        img.write_to(&mut Cursor::new(&mut v), f).unwrap();
        v
    }

    #[test]
    fn any_format_becomes_png_64() {
        for f in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Bmp, ImageFormat::Gif] {
            let src = encode(RgbaImage::from_pixel(300, 200, Rgba([200, 30, 30, 255])), f);
            let c = convert(&src, IconFit::Crop).unwrap();
            assert_eq!((c.source_width, c.source_height), (300, 200));
            assert_eq!(inspect(&c.png).unwrap(), (64, 64, true), "{f:?}");
        }
    }

    #[test]
    fn crop_takes_the_center_and_contain_adds_transparent_bars() {
        // Izquierda roja, centro verde, derecha azul.
        let mut img = RgbaImage::new(300, 100);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = if x < 100 {
                Rgba([255, 0, 0, 255])
            } else if x < 200 {
                Rgba([0, 255, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            };
        }
        let src = encode(img, ImageFormat::Png);
        let crop = image::load_from_memory(&convert(&src, IconFit::Crop).unwrap().png)
            .unwrap()
            .to_rgba8();
        assert_eq!(crop.get_pixel(1, 1)[1], 255);
        assert_eq!(crop.get_pixel(62, 62)[1], 255);
        let contain = image::load_from_memory(&convert(&src, IconFit::Contain).unwrap().png)
            .unwrap()
            .to_rgba8();
        assert_eq!(contain.get_pixel(32, 0)[3], 0, "barra transparente arriba");
        assert_eq!(contain.get_pixel(1, 32)[0], 255, "rojo a la izquierda");
    }

    #[test]
    fn rejects_garbage() {
        assert!(convert(b"no es una imagen", IconFit::Crop).is_err());
    }
}
