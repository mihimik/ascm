use crate::{Header, Frame, ColorPair};
use std::mem::size_of;
use crate::Pixel;

const AVERAGE_COMMAND_SIZE: usize = 7;

#[derive(Debug, Copy, Clone)]
pub enum DeltaCommand {
    UpdatePixel { x: u8, y: u8, ch: char, fg: u8, bg: u8 },
    FillRow { x: u8, y: u8, length: u8, ch: char, fg: u8, bg: u8 },
    // FillCol { x: u8, y: u8, length: u8, ch: char, fg: u8, bg: u8 },
    CopyRegion { src_x: u8, src_y: u8, dst_x: u8, dst_y: u8, w: u8, h: u8 },
    ClearRegion { x: u8, y: u8, w: u8, h: u8 },
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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

    pub fn as_delta(&self) -> Option<&Vec<DeltaCommand>> {
        if let FrameType::Delta { commands } = self {
            Some(commands)
        } else {
            None
        }
    }
}

pub fn get_commands_limit(header: &Header) -> usize {
    let pixels = header.width as usize * header.height as usize;
    let image_size = (size_of::<char>() + size_of::<ColorPair>()) * pixels;

    image_size / AVERAGE_COMMAND_SIZE
}

fn unpack_keyframe_to_dense(header: &Header, keyframe: &Keyframe) -> Vec<Pixel> {
    let total_size = (header.width as usize) * (header.height as usize);
    let mut dense_pixels = Vec::with_capacity(total_size);

    for pixel in &keyframe.pixels {
        if pixel.symbol == '\x1e' {
            let space_count = pixel.color.fg as usize;
            for _ in 0..space_count {
                dense_pixels.push(Pixel {
                    symbol: ' ',
                    color: ColorPair { fg: 0, bg: 0 },
                });
            }
        } else {
            dense_pixels.push(*pixel);
        }
    }

    while dense_pixels.len() < total_size {
        dense_pixels.push(Pixel::space());
    }

    dense_pixels
}

pub fn compare_frames(header: &Header, frame1: &Keyframe, frame2: &Keyframe) -> FrameType {
    let mut commands: Vec<DeltaCommand> = Vec::new();

    let pixels1 = unpack_keyframe_to_dense(header, frame1);
    let pixels2 = unpack_keyframe_to_dense(header, frame2);

    for y in 0..header.height {
        let mut last_pixel = &Pixel::default();
        let mut length = 0;

        for x in 0..header.width {
            let number = (y as usize) * header.width as usize + (x as usize);

            let pixel1 = &pixels1[number];
            let pixel2 = &pixels2[number];

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
    if frames.is_empty() {
        return frames;
    }

    let mut optimized_frames: Vec<Frame> = Vec::new();

    let mut last_keyframe = match &frames[0].frame_type {
        FrameType::Keyframe(kf) => kf.clone(),
        _ => panic!("First frame must be a Keyframe!"),
    };

    optimized_frames.push(frames[0].clone());

    for i in 1..frames.len() {
        let current_frame = &frames[i];

        if let FrameType::Keyframe(ref current_keyframe) = current_frame.frame_type {
            let new_frame_type = compare_frames(&header, &last_keyframe, current_keyframe);

            let mut optimized_frame = current_frame.clone();
            optimized_frame.frame_type = new_frame_type;
            optimized_frames.push(optimized_frame);

            last_keyframe = current_keyframe.clone();
        } else {
            optimized_frames.push(current_frame.clone());
        }
    }

    optimized_frames
}

pub fn convert_commands_to_vec(commands: &Vec<DeltaCommand>) -> Vec<(Pixel, (u8, u8))> {
    let mut pixels = Vec::new();

    for command in commands {
        match *command {
            DeltaCommand::UpdatePixel { x, y, ch, fg, bg } => {
                pixels.push((Pixel::new(ch, ColorPair{ fg, bg }), (x, y)));
            },
            DeltaCommand::FillRow { x, y, length, ch, fg, bg } => {
                let pixel = Pixel::new(ch, ColorPair { fg, bg });
                pixels.extend((0..length).map(|i| (pixel.clone(), (x + i, y))));
            },
            _ => {}
        }
    }

    pixels
}