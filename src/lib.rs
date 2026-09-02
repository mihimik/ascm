mod graphics;
mod file;

use std::collections::HashMap;
use std::path::PathBuf;
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use flate2::Compression;
use crate::graphics::{optimize_frames, FrameType, Keyframe};
use crate::graphics::FrameType::Keyframe as KeyframeType;

const COMPRESSION_MARKER: u8 = 0x1E;

#[derive(Debug, Clone)]
pub struct Header {
    pub magic_bytes: [u8; 4],
    pub version: u8,
    pub width: u8,
    pub height: u8,
}

impl Header {
    pub fn new(width: u8, height: u8) -> Header {
        Self {
            magic_bytes: *b"ASCM",
            version: 1,
            width,
            height,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic_bytes == *b"ASCM" && self.width > 0 && self.height > 0
    }
}

pub fn create_file(path: PathBuf, width: u8, height: u8, frames: Vec<Frame>) -> Result<(), Box<dyn std::error::Error>> {
    let header = Header::new(width, height);

    let file = std::fs::File::create(path)?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    let raw_frames = optimize_frames(header.clone(), frames);

    header.write_to(&mut encoder)?;

    for frame in raw_frames {
        frame.write_to(&mut encoder)?;
    }

    encoder.finish()?;
    Ok(())
}

pub fn read_file(path: PathBuf) -> Result<(Header, Vec<Frame>), Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let mut decoder = GzDecoder::new(file);
    let header = Header::read_from(&mut decoder)?;

    let mut frames = Vec::new();
    loop {
        match Frame::read_from(&mut decoder, &header) {
            Ok(frame) => frames.push(frame),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok((header, frames))
}

#[derive(Clone)]
pub struct Frame {
    pub delay_ms: u32,
    pub frame_type: FrameType,
}

impl Frame {
    pub fn default(resolution: (u8, u8)) -> Frame {
        Self {
            delay_ms: 1,
            frame_type: FrameType::Keyframe(Keyframe::default(resolution)),
        }
    }

    pub fn from_pixels(pixels: HashMap<(u16, u16), Pixel>, delay_ms: u32) -> Frame {
        let mut pairs: Vec<((u16, u16), Pixel)> = pixels.into_iter().collect();
        pairs.sort_unstable_by_key(|&((x, y), _)| (y, x));

        let ordered_pixels: Vec<Pixel> = pairs.into_iter().map(|(_, pixel)| pixel).collect();
        let mut optimized_pixels: Vec<Pixel> = Vec::new();

        let mut i = 0;
        while i < ordered_pixels.len() {
            let current_pixel = &ordered_pixels[i];

            if current_pixel.symbol == ' ' && current_pixel.color.fg == 0 && current_pixel.color.bg == 0 {
                let mut space_count = 1;

                while i + space_count < ordered_pixels.len()
                    && ordered_pixels[i + space_count].symbol == ' '
                    && ordered_pixels[i + space_count].color.fg == 0
                    && ordered_pixels[i + space_count].color.bg == 0
                    && space_count < 255
                {
                    space_count += 1;
                }

                let marker_pixel = Pixel {
                    symbol: '\x1e',
                    color: ColorPair {
                        fg: space_count as u8,
                        bg: 0,
                    },
                };

                optimized_pixels.push(marker_pixel);
                i += space_count;
            } else {
                optimized_pixels.push(current_pixel.clone());
                i += 1;
            }
        }

        let keyframe = Keyframe {
            pixels: ordered_pixels,
        };

        Frame {
            delay_ms,
            frame_type: FrameType::Keyframe(keyframe),
        }
    }
}

#[derive(PartialEq, Copy, Clone, Default)]
pub struct Pixel {
    pub symbol: char,
    pub color: ColorPair,
}

impl Pixel {
    pub fn new(symbol: char, color: ColorPair) -> Pixel {
        Pixel { symbol, color }
    }
    pub fn space() -> Pixel { Pixel { symbol: ' ', color: ColorPair::default() } }
}

#[derive(PartialEq, Copy, Clone, Debug, Default)]
pub struct ColorPair {
    pub fg: u8,
    pub bg: u8,
}

impl ColorPair {
    pub fn white() -> ColorPair {
        Self {
            fg: 15,
            bg: 0,
        }
    }

    pub fn black() -> ColorPair {
        Self {
            fg: 16,
            bg: 0,
        }
    }
}