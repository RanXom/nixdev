use std::env;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    TrueColor,
    Ansi256,
    Ansi16,
    None,
}

impl ColorMode {
    pub fn detect(no_color: bool) -> Self {
        use std::io::IsTerminal;
        let forced = env::var("FORCE_COLOR").is_ok_and(|v| v != "0" && !v.is_empty());
        if no_color || env::var_os("NO_COLOR").is_some() {
            return Self::None;
        }
        if env::var("CLICOLOR").is_ok_and(|v| v == "0") {
            return Self::None;
        }
        if !forced && !std::io::stdout().is_terminal() {
            return Self::None;
        }
        let term = env::var("TERM").unwrap_or_default();
        if term == "dumb" || term.is_empty() {
            return Self::None;
        }
        let colorterm = env::var("COLORTERM").unwrap_or_default().to_lowercase();
        if colorterm.contains("truecolor") || colorterm.contains("24bit") {
            return Self::TrueColor;
        }
        if term.contains("256color") {
            return Self::Ansi256;
        }
        Self::Ansi16
    }

    pub fn colored(self) -> bool {
        self != Self::None
    }
}

pub fn hsl(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0) / 360.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h * 6.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |v: f32| ((v + m) * 255.0).clamp(0.0, 255.0) as u8;
    (to(r), to(g), to(b))
}

fn cube_level(i: u8) -> u8 {
    if i == 0 { 0 } else { 55 + i * 40 }
}

fn distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> i32 {
    let dr = a.0 as i32 - b.0 as i32;
    let dg = a.1 as i32 - b.1 as i32;
    let db = a.2 as i32 - b.2 as i32;
    dr * dr + dg * dg + db * db
}

pub fn to_xterm256(rgb: (u8, u8, u8)) -> u8 {
    let component = |v: u8| ((v as u16 * 5 + 127) / 255) as u8;
    let ri = component(rgb.0);
    let gi = component(rgb.1);
    let bi = component(rgb.2);
    let cube_index = 16 + 36 * ri + 6 * gi + bi;
    let cube_rgb = (cube_level(ri), cube_level(gi), cube_level(bi));

    let avg = (rgb.0 as u16 + rgb.1 as u16 + rgb.2 as u16) / 3;
    let gray_index = if avg > 238 {
        23
    } else {
        avg.saturating_sub(3) / 10
    };
    let gray_index = gray_index.min(23) as u8;
    let level = 8 + gray_index * 10;
    let gray_rgb = (level, level, level);

    if distance(rgb, cube_rgb) <= distance(rgb, gray_rgb) {
        cube_index
    } else {
        232 + gray_index
    }
}

const BASIC: [((u8, u8, u8), u8); 16] = [
    ((0x00, 0x00, 0x00), 0),
    ((0x80, 0x00, 0x00), 1),
    ((0x00, 0x80, 0x00), 2),
    ((0x80, 0x80, 0x00), 3),
    ((0x00, 0x00, 0x80), 4),
    ((0x80, 0x00, 0x80), 5),
    ((0x00, 0x80, 0x80), 6),
    ((0xc0, 0xc0, 0xc0), 7),
    ((0x80, 0x80, 0x80), 8),
    ((0xff, 0x00, 0x00), 9),
    ((0x00, 0xff, 0x00), 10),
    ((0xff, 0xff, 0x00), 11),
    ((0x00, 0x00, 0xff), 12),
    ((0xff, 0x00, 0xff), 13),
    ((0x00, 0xff, 0xff), 14),
    ((0xff, 0xff, 0xff), 15),
];

pub fn to_xterm16(rgb: (u8, u8, u8)) -> u8 {
    BASIC
        .iter()
        .min_by_key(|(c, _)| distance(rgb, *c))
        .map(|(_, i)| *i)
        .unwrap_or(7)
}

pub fn ansi_fg(mode: ColorMode, rgb: (u8, u8, u8), text: &str) -> String {
    let code = match mode {
        ColorMode::TrueColor => format!("38;2;{};{};{}", rgb.0, rgb.1, rgb.2),
        ColorMode::Ansi256 => format!("38;5;{}", to_xterm256(rgb)),
        ColorMode::Ansi16 => ansi16_code(to_xterm16(rgb)).to_string(),
        ColorMode::None => return text.to_string(),
    };
    format!("\x1b[{code}m{text}\x1b[0m")
}

pub fn ansi_bold(mode: ColorMode, text: &str) -> String {
    match mode {
        ColorMode::None => text.to_string(),
        _ => format!("\x1b[1m{text}\x1b[0m"),
    }
}

pub fn ansi_dim(mode: ColorMode, text: &str) -> String {
    match mode {
        ColorMode::None => text.to_string(),
        _ => format!("\x1b[2m{text}\x1b[0m"),
    }
}

fn ansi16_code(index: u8) -> u8 {
    match index {
        0 => 30,
        1 => 31,
        2 => 32,
        3 => 33,
        4 => 34,
        5 => 35,
        6 => 36,
        7 => 37,
        8 => 90,
        9 => 91,
        10 => 92,
        11 => 93,
        12 => 94,
        13 => 95,
        14 => 96,
        _ => 97,
    }
}

pub const ACCENT: (u8, u8, u8) = (0x7a, 0xa2, 0xf7);
pub const ACCENT2: (u8, u8, u8) = (0xbb, 0x9a, 0xf7);
pub const SUCCESS: (u8, u8, u8) = (0x9e, 0xce, 0x6a);
pub const WARN: (u8, u8, u8) = (0xe0, 0xaf, 0x68);
pub const ERROR: (u8, u8, u8) = (0xf7, 0x76, 0x8e);
pub const TEXT: (u8, u8, u8) = (0xc0, 0xca, 0xf5);
pub const MUTED: (u8, u8, u8) = (0x56, 0x5f, 0x89);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xterm256_round_trips_primaries() {
        assert_eq!(to_xterm256((255, 0, 0)), 196);
        assert_eq!(to_xterm256((0, 255, 0)), 46);
        assert_eq!(to_xterm256((0, 0, 255)), 21);
    }

    #[test]
    fn xterm16_picks_red() {
        assert_eq!(to_xterm16((230, 30, 30)), 9);
    }

    #[test]
    fn hsl_wraps_hue() {
        assert_eq!(hsl(360.0, 1.0, 0.5), hsl(0.0, 1.0, 0.5));
        assert_eq!(hsl(0.0, 0.0, 0.5), (127, 127, 127));
    }
}
