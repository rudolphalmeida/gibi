#[derive(Clone, Copy, Debug, Default)]
#[repr(C, packed)]
pub struct RGBA(pub u8, pub u8, pub u8, pub u8);

impl RGBA {
    #[inline(always)]
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        RGBA(r, g, b, a)
    }

    #[inline(always)]
    pub fn full(&self) -> u32 {
        0xFF000000 | ((self.b() as u32) << 16) | ((self.g() as u32) << 8) | (self.r() as u32)
    }

    #[inline(always)]
    pub fn r(&self) -> u8 {
        self.0
    }

    #[inline(always)]
    pub fn g(&self) -> u8 {
        self.1
    }

    #[inline(always)]
    pub fn b(&self) -> u8 {
        self.2
    }
}

#[derive(Clone, Debug)]
pub struct Texture<const WIDTH: usize, const HEIGHT: usize> {
    pub data: [[RGBA; WIDTH]; HEIGHT],
}

impl<const WIDTH: usize, const HEIGHT: usize> Texture<WIDTH, HEIGHT> {
    pub fn pitch(&self) -> usize {
        WIDTH * std::mem::size_of::<RGBA>()
    }

    pub const fn width(&self) -> usize {
        WIDTH
    }

    pub const fn height(&self) -> usize {
        HEIGHT
    }
}

pub trait TextureInfo: Default {
    const HEIGHT: usize;
    const WIDTH: usize;
}

impl<const WIDTH: usize, const HEIGHT: usize> Default for Texture<WIDTH, HEIGHT> {
    fn default() -> Self {
        Self {
            data: [[Default::default(); WIDTH]; HEIGHT],
        }
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> TextureInfo for Texture<WIDTH, HEIGHT> {
    const HEIGHT: usize = HEIGHT;
    const WIDTH: usize = WIDTH;
}
