use std::io::{self, Write, Read};
use crate::graphics::{ColorPair, DeltaCommand, Frame, FrameType, Pixel};
use super::*;

impl Header {
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.magic_bytes)?;
        writer.write_all(&[self.version])?;
        writer.write_all(&[self.width])?;
        writer.write_all(&[self.height])?;
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut magic_bytes = [0u8; 4];
        reader.read_exact(&mut magic_bytes)?;

        let mut buf = [0u8; 3];
        reader.read_exact(&mut buf)?;

        let header = Header {
            magic_bytes,
            version: buf[0],
            width: buf[1],
            height: buf[2],
        };

        if !header.is_valid() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Wrong format ASCM"));
        }

        Ok(header)
    }
}

impl DeltaCommand {
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match *self {
            DeltaCommand::UpdatePixel { x, y, ch, fg, bg } => {
                writer.write_all(&[0x00, x, y])?;
                let mut buf = [0u8; 4];
                let ch_bytes = ch.encode_utf8(&mut buf).as_bytes();
                writer.write_all(&[ch_bytes.len() as u8])?;
                writer.write_all(ch_bytes)?;
                writer.write_all(&[fg, bg])?;
            }
            DeltaCommand::FillRow { x, y, length, ch, fg, bg } => {
                writer.write_all(&[0x01, x, y, length])?;
                let mut buf = [0u8; 4];
                let ch_bytes = ch.encode_utf8(&mut buf).as_bytes();
                writer.write_all(&[ch_bytes.len() as u8])?;
                writer.write_all(ch_bytes)?;
                writer.write_all(&[fg, bg])?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut cmd_id = [0u8; 1];
        reader.read_exact(&mut cmd_id)?;

        match cmd_id[0] {
            0x00 => {
                let mut pos = [0u8; 2];
                reader.read_exact(&mut pos)?;
                let ch = read_char(reader)?;
                let mut colors = [0u8; 2];
                reader.read_exact(&mut colors)?;

                Ok(DeltaCommand::UpdatePixel {
                    x: pos[0],
                    y: pos[1],
                    ch,
                    fg: colors[0],
                    bg: colors[1],
                })
            }
            0x01 => {
                let mut buf = [0u8; 3];
                reader.read_exact(&mut buf)?;
                let ch = read_char(reader)?;
                let mut colors = [0u8; 2];
                reader.read_exact(&mut colors)?;

                Ok(DeltaCommand::FillRow {
                    x: buf[0],
                    y: buf[1],
                    length: buf[2],
                    ch,
                    fg: colors[0],
                    bg: colors[1],
                })
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Неизвестная дельта-команда: {}", cmd_id[0]),
            )),
        }
    }
}

impl Frame {
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.delay_ms.to_le_bytes())?;

        match &self.frame_type {
            FrameType::Keyframe(keyframe) => {
                writer.write_all(&[0x00])?;
                for i in 0..keyframe.pixels.len() {
                    let mut buf = [0u8; 4];
                    let ch_bytes = keyframe.pixels[i].symbol.encode_utf8(&mut buf).as_bytes();
                    writer.write_all(&[ch_bytes.len() as u8])?;
                    writer.write_all(ch_bytes)?;
                    writer.write_all(&[keyframe.pixels[i].color.fg, keyframe.pixels[i].color.bg])?;
                }
            }
            FrameType::Delta { commands } => {
                writer.write_all(&[0x01])?;
                let count = commands.len() as u16;
                writer.write_all(&count.to_le_bytes())?;
                for cmd in commands {
                    cmd.write_to(writer)?;
                }
            }
        }
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R, header: &Header) -> io::Result<Self> {
        let mut delay_bytes = [0u8; 4];
        reader.read_exact(&mut delay_bytes)?;
        let delay_ms = u32::from_le_bytes(delay_bytes);

        let mut type_byte = [0u8; 1];
        reader.read_exact(&mut type_byte)?;

        let frame_type = match type_byte[0] {
            0x00 => {
                let total_pixels = (header.width as usize) * (header.height as usize);
                let mut pixels = Vec::with_capacity(total_pixels);

                for _ in 0..total_pixels {
                    let ch = read_char(reader)?;
                    let mut col_buf = [0u8; 2];
                    reader.read_exact(&mut col_buf)?;

                    pixels.push(Pixel {
                        symbol: ch,
                        color: ColorPair {
                            fg: col_buf[0],
                            bg: col_buf[1],
                        },
                    })
                }

                FrameType::Keyframe(Keyframe { pixels })
            }
            0x01 => {
                let mut count_bytes = [0u8; 2];
                reader.read_exact(&mut count_bytes)?;
                let command_count = u16::from_le_bytes(count_bytes) as usize;

                let mut commands = Vec::with_capacity(command_count);
                for _ in 0..command_count {
                    commands.push(DeltaCommand::read_from(reader)?);
                }

                FrameType::Delta { commands }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Неизвестный тип кадра: {}", type_byte[0]),
                ));
            }
        };

        Ok(Frame { delay_ms, frame_type })
    }
}

fn read_char<R: Read>(reader: &mut R) -> io::Result<char> {
    let mut len_buf = [0u8; 1];
    reader.read_exact(&mut len_buf)?;
    let len = len_buf[0] as usize;

    let mut char_buf = vec![0u8; len];
    reader.read_exact(&mut char_buf)?;

    let s = std::str::from_utf8(&char_buf)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "UTF-8 error"))?;

    s.chars().next().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Empty symbol"))
}