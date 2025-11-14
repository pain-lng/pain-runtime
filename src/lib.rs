// Pain runtime library

pub mod allocator;
pub mod object;

pub use allocator::{Arena, BumpAllocator};
pub use object::{Object, Runtime, Value};

