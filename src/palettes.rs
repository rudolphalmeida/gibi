use crate::textures::RGBA;

enum GameboyColorShade {
    White = 0,
    LightGray = 1,
    DarkGray = 2,
    Black = 3,
}

impl GameboyColorShade {
    pub fn new(bits: u8) -> Self {
        match bits {
            0 => GameboyColorShade::White,
            1 => GameboyColorShade::LightGray,
            2 => GameboyColorShade::DarkGray,
            3 => GameboyColorShade::Black,
            _ => panic!("Invalid bits for Color shade"),
        }
    }
}

pub(crate) const RGBA_WHITE: RGBA = RGBA(0xE0, 0xF8, 0xD0, 0xFF);
pub(crate) const RGBA_LIGHT_GRAY: RGBA = RGBA(0x88, 0xC0, 0x70, 0xFF);
pub(crate) const RGBA_DARK_GRAY: RGBA = RGBA(0x34, 0x68, 0x56, 0xFF);
pub(crate) const RGBA_BLACK: RGBA = RGBA(0x08, 0x18, 0x20, 0xFF);

fn map_to_actual_color(shade: GameboyColorShade) -> RGBA {
    match shade {
        GameboyColorShade::White => RGBA_WHITE,
        GameboyColorShade::LightGray => RGBA_LIGHT_GRAY,
        GameboyColorShade::DarkGray => RGBA_DARK_GRAY,
        GameboyColorShade::Black => RGBA_BLACK,
    }
}

fn extract_actual_color_from_spec(spec: &[u8; 8], index: usize) -> RGBA {
    let color_byte_1 = spec[index * 2];
    let color_byte_2 = spec[index * 2 + 1];

    // GGGRRRRR             |  XBBBBBGG
    // Color 1              |  Color 2
    // Red and Lower Green  |  Upper Green and Blue

    let r = color_byte_1 & 0b11111;
    let g = ((color_byte_2 & 0b11) << 3) | ((color_byte_1 & 0b11100000) >> 5);
    let b = (color_byte_2 & 0b01111100) >> 2;

    // RGB555 to RGB888: https://stackoverflow.com/a/4409837/4681203
    RGBA::new(
        (r << 3) | (r >> 2),
        (g << 3) | (g >> 2),
        (b << 3) | (b >> 2),
        0xFF,
    )
}

pub(crate) enum Palette {
    DmgGreyscale(u8),
    CgbColor([u8; 8]),
}

impl Palette {
    pub fn new_greyscale(value: u8) -> Self {
        Palette::DmgGreyscale(value)
    }

    pub fn new_color(values: &[u8]) -> Self {
        let mut palette = [0xFF; 8];
        palette.copy_from_slice(values);

        Palette::CgbColor(palette)
    }

    pub fn actual_color_from_index(&self, index: u8) -> RGBA {
        match index {
            0 => self.color0(),
            1 => self.color1(),
            2 => self.color2(),
            3 => self.color3(),
            _ => panic!("Invalid index for color palette {}", index),
        }
    }

    pub fn color0(&self) -> RGBA {
        match self {
            Palette::DmgGreyscale(value) => {
                map_to_actual_color(GameboyColorShade::new(value & 0x03))
            }
            Palette::CgbColor(spec) => extract_actual_color_from_spec(spec, 0),
        }
    }

    pub fn color1(&self) -> RGBA {
        match self {
            Palette::DmgGreyscale(value) => {
                map_to_actual_color(GameboyColorShade::new((value & 0x0C) >> 2))
            }
            Palette::CgbColor(spec) => extract_actual_color_from_spec(spec, 1),
        }
    }

    pub fn color2(&self) -> RGBA {
        match self {
            Palette::DmgGreyscale(value) => {
                map_to_actual_color(GameboyColorShade::new((value & 0x30) >> 4))
            }
            Palette::CgbColor(spec) => extract_actual_color_from_spec(spec, 2),
        }
    }

    pub fn color3(&self) -> RGBA {
        match self {
            Palette::DmgGreyscale(value) => {
                map_to_actual_color(GameboyColorShade::new((value & 0xC0) >> 6))
            }
            Palette::CgbColor(spec) => extract_actual_color_from_spec(spec, 3),
        }
    }
}
