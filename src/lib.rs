// Pain runtime library

pub mod allocator;
pub mod gc;
pub mod object;

pub use allocator::{Arena, BumpAllocator};
pub use gc::GarbageCollector;
pub use object::{Object, Runtime, Value, ClassInstance};

