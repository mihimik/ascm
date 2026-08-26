use std::cmp::PartialEq;
use crate::{Header, Frame, ColorPair};
use std::mem::size_of;
use std::collections::HashMap;
use crate::Pixel;

const AVERAGE_COMMAND_SIZE: usize = 7;

#[derive(Debug, Copy, Clone)]
pub enum DeltaCommand {
    UpdatePixel { x: u8, y: u8, ch: char, fg: u8, bg: u8 },
    FillRow { x: u8, y: u8, length: u8, ch: char, fg: u8, bg: u8 },
    FillCol { x: u8, y: u8, length: u8, ch: char, fg: u8, bg: u8 },
    CopyRegion { src_x: u8, src_y: u8, dst_x: u8, dst_y: u8, w: u8, h: u8 },
    ClearRegion { x: u8, y: u8, w: u8, h: u8 },
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

pub fn optimize_frames(header: Header, mut frames: Vec<Frame>) -> Vec<Frame> {
    let mut last_frame = Frame::default((header.width, header.height));
    let mut optimized_frames: Vec<Frame> = Vec::new();

    for (i, frame) in frames.iter_mut().enumerate() {
        if let FrameType::Keyframe(ref keyframe) = last_frame.frame_type {

            if i == 0 {
                optimized_frames.push(frame.clone());
                last_frame = frame.clone();
            } else {
                let mut new_frame = frame.clone();
                new_frame.frame_type = compare_frames(&header, &last_frame.frame_type.as_keyframe().expect("Frame must be Keyframe"), &keyframe);
                optimized_frames.push(new_frame);
                last_frame = frame.clone();
            }
        }
    }

    optimized_frames
}