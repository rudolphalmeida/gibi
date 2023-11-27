use crate::textures::TextureInfo;

// TODO: Implement this module for your own or find a library to do it
//       This is copied from https://github.com/Kim-Dewelski/bitwolf-archived

pub mod access;
pub mod buffers;

trait Buffer<T: TextureInfo> {
    fn read(&mut self) -> &T;

    fn reader_drop(&mut self);

    fn write(&mut self) -> &mut T;

    fn writer_drop(&mut self);
}
