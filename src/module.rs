use std::os::unix::io::RawFd;
use std::time::Instant;

pub(crate) struct Block {
    /// The text in the block.
    pub text: String,

    /// This will display the block in red.
    pub is_warning: bool,
}

pub(crate) trait Module {
    /// Render into a list of `Block`s.
    fn render<'a>(&'a self) -> Box<dyn Iterator<Item = Block> + 'a>;

    /// This method is called when the `Module` needs to update. See
    /// `pollable_fd` and `timeout`. It returns `true` if the blocks need to be
    /// rerendered.
    fn update(&mut self) -> bool;

    /// Returns a file descriptor which should be polled. When the file
    /// descriptor is ready to be read from, the `Module`'s `update` method is
    /// called.
    fn pollable_fd(&self) -> Option<RawFd>;

    /// If this method returns some instant, then the module should be updated
    /// before that instant.
    fn timeout(&self) -> Option<Instant>;
}
