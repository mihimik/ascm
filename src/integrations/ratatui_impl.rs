use std::time::{Instant, Duration};
use ratatui::widgets::Widget;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::graphics::{convert_commands_to_vec, FrameType};
use crate::{Frame, Pixel, ColorPair};

pub struct AscmViewState {
    start_time: Instant,
    dimensions: (u8, u8),
    frames: Vec::<Frame>,
    current_frame: usize,
    current_buffer: Vec<Pixel>,
}

impl AscmViewState {
    pub fn new(dimensions: (u8, u8), frames: Vec<Frame>) -> Self {
        let total_pixels = (dimensions.0 as usize) * (dimensions.1 as usize);
        let current_buffer = vec![
            Pixel { symbol: ' ', color: ColorPair { fg: 0, bg: 0 } };
            total_pixels
        ];

        let mut selff = Self {
            start_time: Instant::now(),
            dimensions,
            frames,
            current_frame: 0,
            current_buffer,
        };

        selff.apply_current_frame_to_buffer();
        selff
    }

    pub fn ask(&mut self) {
        let needle_delay = Duration::from_millis(self.frames[self.current_frame].delay_ms as u64);

        if self.start_time.elapsed() >= needle_delay {
            self.start_time = Instant::now();
            self.current_frame = (self.current_frame + 1) % self.frames.len();
            self.apply_current_frame_to_buffer();
        }
    }

    fn apply_current_frame_to_buffer(&mut self) {
        let frame = &self.frames[self.current_frame];
        let width = self.dimensions.0 as usize;

        match &frame.frame_type {
            FrameType::Keyframe(keyframe) => {
                let mut idx = 0;
                for pixel in &keyframe.pixels {
                    if pixel.symbol == '\x1e' {
                        let space_count = pixel.color.fg as usize;
                        for _ in 0..space_count {
                            if idx < self.current_buffer.len() {
                                self.current_buffer[idx] = Pixel { symbol: ' ', color: ColorPair { fg: 0, bg: 0 } };
                                idx += 1;
                            }
                        }
                    } else {
                        if idx < self.current_buffer.len() {
                            self.current_buffer[idx] = *pixel;
                            idx += 1;
                        }
                    }
                }
            }
            FrameType::Delta { commands } => {
                let pixels = convert_commands_to_vec(commands);
                for (pixel, (x, y)) in pixels {
                    let idx = (y as usize) * width + (x as usize);
                    if idx < self.current_buffer.len() {
                        self.current_buffer[idx] = pixel;
                    }
                }
            }
        }
    }
}

pub struct AscmView<'a> {
    pub state: &'a mut AscmViewState,
}

impl<'a> Widget for AscmView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.state.ask();

        let dimensions = self.state.dimensions;
        let render_w = (dimensions.0 as u16).min(area.width);
        let render_h = (dimensions.1 as u16).min(area.height);

        for y in 0..render_h {
            for x in 0..render_w {
                let idx = (y as usize) * (dimensions.0 as usize) + (x as usize);

                if let Some(pixel) = self.state.current_buffer.get(idx) {
                    let target_x = area.x + x;
                    let target_y = area.y + y;

                    let cell = buf.get_mut(target_x, target_y);
                    cell.set_char(pixel.symbol);

                    let mut style = Style::default();
                    if pixel.color.fg != 0 {
                        style = style.fg(Color::Indexed(pixel.color.fg));
                    }
                    if pixel.color.bg != 0 {
                        style = style.bg(Color::Indexed(pixel.color.bg));
                    }
                    cell.set_style(style);
                }
            }
        }
    }
}
