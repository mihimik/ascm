use std::cmp::PartialEq;
use crate::Header;
use std::mem::size_of;
use std::collections::HashMap;

#[derive(PartialEq, Copy, Clone)]
pub struct ColorPair {
    pub fg: u8,
    pub bg: u8,
}

impl ColorPair {
    fn default() -> ColorPair {
        Self {
            fg: 0,
            bg: 0,
        }
    }
}

const AVERAGE_COMMAND_SIZE: usize = 7;

#[derive(Debug, Copy, Clone)]
pub enum DeltaCommand {
    UpdatePixel { x: u8, y: u8, ch: char, fg: u8, bg: u8 },
    FillRow { x: u8, y: u8, length: u8, ch: char, fg: u8, bg: u8 },
    FillCol { x: u8, y: u8, length: u8, ch: char, fg: u8, bg: u8 },
    CopyRegion { src_x: u8, src_y: u8, dst_x: u8, dst_y: u8, w: u8, h: u8 },
    ClearRegion { x: u8, y: u8, w: u8, h: u8 },
}

#[derive(PartialEq, Copy, Clone)]
pub struct Pixel {
    pub symbol: char,
    pub color: ColorPair,
}

impl Pixel {
    pub fn new(symbol: char, color: ColorPair) -> Pixel {
        Pixel { symbol, color }
    }

    pub fn default() -> Pixel {
        Self {
            symbol: char::default(),
            color: ColorPair::default(),
        }
    }
}

#[derive(Clone)]
pub struct Keyframe {
    pub pixels: Vec<Pixel>,
}

impl Keyframe {
    pub fn default(resolution: (u8, u8)) -> Keyframe {
        let pixels = vec![Pixel::default(); resolution.0 as usize * resolution.1 as usize];
        Self {
            pixels,
        }
    }
}

#[derive(Clone)]
pub enum FrameType {
    Keyframe(Keyframe),
    Delta {
        commands: Vec<DeltaCommand>,
    }
}

impl FrameType {
    pub fn as_keyframe(&self) -> Option<&Keyframe> {
        if let FrameType::Keyframe(k) = self {
            Some(k)
        } else {
            None
        }
    }
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
}

pub fn get_commands_limit(header: &Header) -> usize {
    let pixels = (header.width * header.height) as usize;
    let image_size = (size_of::<char>() + size_of::<ColorPair>()) * pixels;

    image_size / AVERAGE_COMMAND_SIZE
}

pub fn compare_frames(header: &Header, frame1: &Keyframe, frame2: &Keyframe) -> FrameType {
    let mut commands: Vec<DeltaCommand> = Vec::new();

    for y in 0..header.height {
        let mut last_pixel = Pixel::default();
        let mut length = 0;

        for x in 0..header.width {
            let number = (y * header.width + x) as usize;

            let pixel1 = frame1.pixels[number];
            let pixel2 = frame2.pixels[number];

            if pixel1 != pixel2 {
                if length == 0 {
                    last_pixel = pixel2;
                    length = 1;
                } else if pixel2 == last_pixel {
                    length += 1;
                } else {
                    if length == 1 {
                        commands.push(DeltaCommand::UpdatePixel { x: x - 1, y, ch: last_pixel.symbol, fg: last_pixel.color.fg, bg: last_pixel.color.bg });
                    } else {
                        commands.push(DeltaCommand::FillRow { x: x - length, y, length, ch: last_pixel.symbol, fg: last_pixel.color.fg, bg: last_pixel.color.bg });
                    }
                    last_pixel = pixel2;
                    length = 1;
                }
            } else {
                if length > 0 {
                    if length == 1 {
                        commands.push(DeltaCommand::UpdatePixel { x: x - 1, y, ch: last_pixel.symbol, fg: last_pixel.color.fg, bg: last_pixel.color.bg });
                    } else {
                        commands.push(DeltaCommand::FillRow { x: x - length, y, length, ch: last_pixel.symbol, fg: last_pixel.color.fg, bg: last_pixel.color.bg });
                    }
                    length = 0;
                }
            }
        }

        if length == 1 {
            commands.push(DeltaCommand::UpdatePixel {
                x: header.width - 1, y,
                ch: last_pixel.symbol,
                fg: last_pixel.color.fg, bg: last_pixel.color.bg
            });
        } else if length > 1 {
            commands.push(DeltaCommand::FillRow {
                x: header.width - length, y,
                length, ch: last_pixel.symbol,
                fg: last_pixel.color.fg, bg: last_pixel.color.bg
            });
        }
    }

    if commands.len() < get_commands_limit(header) {
        return FrameType::Delta { commands }
    }

    FrameType::Keyframe(frame2.clone())
}