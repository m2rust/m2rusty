//! Image decoding: port of `neoeter/src/image` (`CImage`, `CTGAImage`,
//! `CDXTCImage`), of the stb_image based `Platform_GetImageInfo` /
//! `Platform_DecodeImage` in `neoeter/src/platform/base/ImageLoaderCompat.cpp`
//! and of the file-to-pixels paths of `TargaResource.cpp`.
//!
//! Decoding only: no textures, no device. Pixels are `0xAARRGGBB` words as in
//! the C++ code (`DWORD`), or BGRA bytes where the C++ API says so.
//!
//! Modules:
//! * [`Image`] -- `CImage`, a `width * height` surface of `u32` pixels.
//! * [`tga`] -- `CTGAImage`, the client's own TGA reader and writer.
//! * [`dxt`] -- `CDXTCImage`, DDS header parsing and DXT1/3/5 decompression.
//! * [`stb`] -- the parts of stb_image the client links (JPEG, TGA, BMP, PNG).
//! * [`loader`] -- `Platform_GetImageInfo` / `Platform_DecodeImage`.

// The stb ports keep the C index loops and constants verbatim, so they are diffable against stb_image.h.
#![allow(
    clippy::excessive_precision,
    clippy::needless_range_loop,
    clippy::int_plus_one,
    clippy::too_many_arguments,
    clippy::chunks_exact_to_as_chunks,
    clippy::manual_range_contains,
    clippy::unnecessary_cast,
    clippy::manual_div_ceil,
    clippy::needless_late_init
)]

pub mod dxt;
pub mod loader;
pub mod stb;
pub mod tga;

pub use dxt::{DxtcImage, PixFormat};
pub use loader::{DecodedImage, ImageInfo, decode_image, get_image_info};
pub use tga::{TgaHeader, TgaImage};

/// `CImage`: a `width * height` surface of `0xAARRGGBB` pixels, row 0 first.
///
/// `CImage::Create` leaves the pixels uninitialised; here they start as 0.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Image {
    width: i32,
    height: i32,
    pixels: Vec<u32>,
    created: bool,
}

impl Image {
    /// `CImage::CImage()` + `Create(width, height)`.
    ///
    /// # Panics
    /// If a dimension is negative.
    pub fn new(width: i32, height: i32) -> Self {
        assert!(width >= 0 && height >= 0, "negative image size {width}x{height}");
        Image { width, height, pixels: vec![0; width as usize * height as usize], created: true }
    }

    /// Wraps existing pixels (`pixels.len() == width * height`).
    pub(crate) fn from_pixels(width: i32, height: i32, pixels: Vec<u32>) -> Self {
        debug_assert_eq!(pixels.len(), width as usize * height as usize);
        Image { width, height, pixels, created: true }
    }

    /// `CImage::IsEmpty`: true until a surface was created (a 0x0 surface is not empty).
    pub fn is_empty(&self) -> bool {
        !self.created
    }

    /// `CImage::GetWidth`.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// `CImage::GetHeight`.
    pub fn height(&self) -> i32 {
        self.height
    }

    /// `CImage::GetBasePointer`.
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// `CImage::GetBasePointer`, mutable.
    pub fn pixels_mut(&mut self) -> &mut [u32] {
        &mut self.pixels
    }

    /// `CImage::GetLinePointer`: row `line`.
    pub fn line(&self, line: i32) -> &[u32] {
        let w = self.width as usize;
        &self.pixels[line as usize * w..(line as usize + 1) * w]
    }

    /// `CImage::GetLinePointer`, mutable.
    pub fn line_mut(&mut self, line: i32) -> &mut [u32] {
        let w = self.width as usize;
        &mut self.pixels[line as usize * w..(line as usize + 1) * w]
    }

    /// `CImage::Clear`.
    pub fn clear(&mut self, color: u32) {
        self.pixels.fill(color);
    }

    /// `CImage::PutImage`: copies `src` with its top-left at (`x`, `y`).
    ///
    /// # Panics
    /// If `src` does not fit (the C++ `assert`).
    pub fn put_image(&mut self, x: i32, y: i32, src: &Image) {
        assert!(x >= 0 && x + src.width <= self.width);
        assert!(y >= 0 && y + src.height <= self.height);
        for j in 0..src.height {
            let dest = &mut self.line_mut(y + j)[x as usize..x as usize + src.width as usize];
            dest.copy_from_slice(src.line(j));
        }
    }

    /// `CImage::FlipTopToBottom`: mirrors the rows.
    pub fn flip_top_to_bottom(&mut self) {
        let w = self.width as usize;
        let h = self.height as usize;
        for row in 0..h / 2 {
            let (top, bottom) = self.pixels.split_at_mut((h - row - 1) * w);
            top[row * w..(row + 1) * w].swap_with_slice(&mut bottom[..w]);
        }
    }
}

/// Zero-filled pixel buffer; `None` if the allocation is refused (hostile
/// headers can ask for gigabytes where the C++ code would crash or read
/// garbage).
pub(crate) fn alloc_pixels(count: usize) -> Option<Vec<u32>> {
    let mut v = Vec::new();
    v.try_reserve_exact(count).ok()?;
    v.resize(count, 0);
    Some(v)
}
