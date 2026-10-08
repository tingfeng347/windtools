use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

#[derive(Clone, Copy)]
struct Blade {
    x: u16,
    y: u16,
    height: u16,
    phase: f64,
    depth: f64,
}

pub struct Meadow {
    pub seed: u64,
    pub paused: bool,
    pub direction: f64,
    pub time: f64,
    gust: f64,
    size: (u16, u16),
    far: Vec<u16>,
    near: Vec<u16>,
    blades: Vec<Blade>,
}
impl Meadow {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            paused: false,
            direction: 1.0,
            time: 0.0,
            gust: -100.0,
            size: (0, 0),
            far: Vec::new(),
            near: Vec::new(),
            blades: Vec::new(),
        }
    }
    pub fn tick(&mut self, delta: f64) {
        if !self.paused {
            self.time += delta.min(0.1);
        }
    }
    pub fn gust(&mut self) {
        self.gust = self.time;
    }
    pub fn wind(&self) -> f64 {
        self.direction
            * (0.6
                + ((self.time * 0.6).sin() + 1.0) * 0.15
                + (-(self.time - self.gust) * 0.9).exp().min(1.0) * 2.4)
    }
    pub fn prepare(&mut self, width: u16, height: u16) {
        if self.size == (width, height) {
            return;
        }
        self.size = (width, height);
        self.far.clear();
        self.near.clear();
        self.blades.clear();
        for x in 0..width {
            let position = x as f64 / f64::from(width.max(1));
            let ridge = (position * 9.0 + self.seed as f64 * 0.01).sin() * 0.055
                + (position * 23.0).sin() * 0.025;
            self.far.push(((0.34 + ridge) * height as f64) as u16);
            self.near.push(
                ((0.48 + (position * 12.0 + 0.8).sin() * 0.045 + (position * 4.0).cos() * 0.04)
                    * height as f64) as u16,
            );
        }
        let mut rng = self.seed.max(1);
        let count = (usize::from(width) * usize::from(height) / 5).min(1800);
        for _ in 0..count {
            let x = (random(&mut rng) * f64::from(width.max(1))) as u16;
            let y = ((0.60 + random(&mut rng) * 0.40) * height as f64) as u16;
            let depth = y as f64 / f64::from(height.max(1));
            self.blades.push(Blade {
                x,
                y: y.min(height.saturating_sub(1)),
                height: (1.0 + depth * depth * random(&mut rng) * 5.0) as u16,
                phase: random(&mut rng) * std::f64::consts::TAU,
                depth,
            });
        }
        self.blades.sort_by_key(|blade| blade.y);
    }
}
fn random(state: &mut u64) -> f64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 11) as f64 / ((1_u64 << 53) as f64)
}

pub struct MeadowView<'a> {
    pub meadow: &'a Meadow,
    pub ascii: bool,
    pub mono: bool,
    pub ansi: bool,
}
impl MeadowView<'_> {
    fn color(&self, rgb: (u8, u8, u8), fallback: Color) -> Color {
        if self.mono {
            Color::Reset
        } else if self.ansi {
            fallback
        } else {
            Color::Rgb(rgb.0, rgb.1, rgb.2)
        }
    }
}
impl Widget for MeadowView<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let sky = self.color((16, 29, 47), Color::Black);
        let far = self.color((44, 69, 84), Color::DarkGray);
        let near = self.color((36, 73, 71), Color::Blue);
        let ground = self.color((18, 45, 38), Color::Black);
        for y in 0..area.height {
            for x in 0..area.width {
                let far_y = self
                    .meadow
                    .far
                    .get(x as usize)
                    .copied()
                    .unwrap_or(area.height / 3);
                let near_y = self
                    .meadow
                    .near
                    .get(x as usize)
                    .copied()
                    .unwrap_or(area.height / 2);
                let cell = &mut buffer[(area.x + x, area.y + y)];
                let background = if y < far_y {
                    sky
                } else if y < near_y {
                    far
                } else if y < area.height * 3 / 5 {
                    near
                } else {
                    ground
                };
                cell.set_symbol(" ").set_bg(background);
                if y == far_y || y == near_y {
                    cell.set_symbol(if self.ascii { "_" } else { "▄" })
                        .set_fg(if y == far_y { far } else { near });
                } else if self.mono && y > far_y && y < area.height * 3 / 5 && (x + y * 3) % 11 == 0
                {
                    cell.set_symbol(".");
                }
            }
        }
        let wind = self.meadow.wind();
        for blade in &self.meadow.blades {
            let bend = (self.meadow.time * 1.7 - blade.x as f64 * 0.09 + blade.phase).sin() * 0.35
                + wind * (0.45 + blade.depth * 0.35);
            let color = self.color(
                (
                    (78.0 + blade.depth * 45.0) as u8,
                    (111.0 + blade.depth * 53.0) as u8,
                    (77.0 + blade.depth * 34.0) as u8,
                ),
                Color::Green,
            );
            for segment in 0..blade.height {
                let shift =
                    (bend * (segment as f64 / blade.height as f64).powi(2) * 3.0).round() as i32;
                let x = blade.x as i32 + shift;
                let y = blade.y as i32 - segment as i32;
                if x >= 0 && y >= 0 && x < area.width as i32 && y < area.height as i32 {
                    let symbol = if bend > 0.55 {
                        if self.ascii { "/" } else { "╱" }
                    } else if bend < -0.55 {
                        if self.ascii { "\\" } else { "╲" }
                    } else if self.ascii {
                        "|"
                    } else {
                        "│"
                    };
                    buffer[(area.x + x as u16, area.y + y as u16)]
                        .set_symbol(symbol)
                        .set_fg(color);
                }
            }
        }
        let leaf_color = self.color((219, 188, 112), Color::Yellow);
        for index in 0..(area.width / 5).clamp(3, 40) {
            let phase = index as f64 * 7.37 + (self.meadow.seed % 97) as f64;
            let x = (phase + self.meadow.time * (2.0 + index as f64 % 4.0) * self.meadow.direction)
                .rem_euclid(area.width as f64) as u16;
            let y = ((0.42 + (phase + self.meadow.time * 0.8).sin() * 0.15) * area.height as f64)
                as u16;
            if y < area.height {
                buffer[(area.x + x, area.y + y)]
                    .set_symbol(if self.ascii { "'" } else { "·" })
                    .set_fg(leaf_color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paused_scene_and_ascii_are_stable_at_small_sizes() {
        let mut scene = Meadow::new(42);
        scene.paused = true;
        scene.tick(10.0);
        assert_eq!(scene.time, 0.0);
        for (width, height) in [(1, 1), (40, 8), (120, 35), (240, 75)] {
            scene.prepare(width, height);
            let area = Rect::new(0, 0, width, height);
            let mut buffer = Buffer::empty(area);
            MeadowView {
                meadow: &scene,
                ascii: true,
                mono: false,
                ansi: false,
            }
            .render(area, &mut buffer);
            assert!(buffer.content.iter().all(|cell| cell.symbol().is_ascii()));
        }
    }
}
