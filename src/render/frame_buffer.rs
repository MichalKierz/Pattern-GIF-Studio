use anyhow::{Result, ensure};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; pixel_len(width, height)],
        }
    }

    pub fn from_pixels(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self> {
        ensure!(
            pixels.len() == pixel_len(width, height),
            "frame buffer length does not match RGBA8 dimensions"
        );
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        let offset = ((y * self.width + x) * 4) as usize;
        self.pixels[offset..offset + 4].copy_from_slice(&rgba);
    }
}

pub fn pixel_len(width: u32, height: u32) -> usize {
    width as usize * height as usize * 4
}
