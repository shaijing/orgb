use anyhow::Result;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Rgb {
    pub const BLACK: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pixels: Vec<Rgb>,
}

impl Frame {
    pub fn solid(led_count: usize, color: Rgb) -> Self {
        Self {
            pixels: vec![color; led_count],
        }
    }

    pub fn from_pixels(pixels: Vec<Rgb>) -> Self {
        Self { pixels }
    }

    pub fn pixels(&self) -> &[Rgb] {
        &self.pixels
    }

    pub fn len(&self) -> usize {
        self.pixels.len()
    }
}

pub trait LightingBackend {
    fn led_count(&self) -> usize;

    fn min_frame_interval(&self) -> Duration;

    fn send_frame(&mut self, frame: &Frame) -> Result<()>;

    fn set_color(&mut self, color: Rgb) -> Result<()> {
        let frame = Frame::solid(self.led_count(), color);
        self.send_frame(&frame)
    }
}
