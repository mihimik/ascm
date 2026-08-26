mod graphics;
mod file;

use std::collections::HashMap;
use std::path::PathBuf;
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use flate2::Compression;
use crate::graphics::{optimize_frames, FrameType, Keyframe};
use crate::graphics::FrameType::Keyframe as KeyframeType;

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

pub fn read_file(name: String) -> Result<(Header, Vec<Frame>), Box<dyn std::error::Error>> {
    let file = std::fs::File::open(format!("{}.ascm", name))?;
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
        let keyframe = Keyframe {
            pixels: pixels.values().copied().collect(),
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
}

#[derive(PartialEq, Copy, Clone, Debug, Default)]
pub struct ColorPair {
    pub fg: u8,
    pub bg: u8,
}

impl ColorPair {
    pub fn default() -> ColorPair {
        Self {
            fg: 0,
            bg: 0,
        }
    }
}