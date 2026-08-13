mod graphics;
mod file;

use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use flate2::Compression;
use crate::graphics::{compare_frames, Frame, FrameType, Keyframe};
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

pub fn create_file(name: String, width: u8, height: u8, frames: Vec<Frame>) -> Result<(), Box<dyn std::error::Error>> {
    let header = Header::new(width, height);

    let file = std::fs::File::create(format!("{}.ascm", name))?;
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

fn optimize_frames(header: Header, mut frames: Vec<Frame>) -> Vec<Frame> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use std::io::Read;
    use tempfile::tempdir;
    use crate::graphics::{ColorPair, Pixel};

    fn create_dummy_keyframe(width: u8, height: u8, ch: char) -> Keyframe {
        let size = (width as usize) * (height as usize);
        let pixel = Pixel {
            symbol: ch,
            color: ColorPair { fg: 1, bg: 0 }
        };
        Keyframe {
            pixels: vec![pixel; size],
        }
    }

    #[test]
    fn test_optimize_frames_converts_to_delta() {
        let header = Header::new(10, 10);

        let kf1 = create_dummy_keyframe(10, 10, 'A');
        let mut kf2 = create_dummy_keyframe(10, 10, 'A');
        kf2.pixels[0].symbol = 'B';

        let frames = vec![
            Frame { delay_ms: 100, frame_type: FrameType::Keyframe(kf1) },
            Frame { delay_ms: 100, frame_type: FrameType::Keyframe(kf2) },
        ];

        let optimized = optimize_frames(header, frames);

        assert_eq!(optimized.len(), 2);

        assert!(matches!(optimized[0].frame_type, FrameType::Keyframe(_)));

        assert!(matches!(optimized[1].frame_type, FrameType::Delta { .. }));
    }

    #[test]
    fn test_create_file_writes_valid_gzip() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        // let file_path = dir.path().join("test_anim");
        let file_path = "test_anim";

        let header = Header::new(2, 2);
        let color = ColorPair { fg: 1, bg: 2 };
        let original_kf = Keyframe {
            pixels: vec![Pixel{symbol:'A',color},Pixel{symbol:'B',color},Pixel{symbol:'C',color},Pixel{symbol:'D',color}],
        };
        let original_kf2 = Keyframe {
            pixels: vec![Pixel{symbol:'B',color},Pixel{symbol:'B',color},Pixel{symbol:'C',color},Pixel{symbol:'D',color}],
        };
        let original_frames = vec![
            Frame { delay_ms: 100, frame_type: FrameType::Keyframe(original_kf) },
            Frame { delay_ms: 100, frame_type: FrameType::Keyframe(original_kf2) },
        ];

        create_file(file_path.to_string(), header.width, header.height, original_frames)?;

        let (read_header, read_frames) = read_file(file_path.to_string())?;

        assert_eq!(read_header.width, header.width);
        assert_eq!(read_header.height, header.height);
        assert_eq!(read_frames.len(), 2);
        assert_eq!(read_frames[0].delay_ms, 100);

        println!("Round-trip успешно пройден! Прочитано кадров: {}", read_frames.len());

        Ok(())
    }
}