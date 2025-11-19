// Basic allocator module - bump allocator and arena allocator

use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

/// Bump allocator - simple linear allocator for fast allocation
/// Allocations are not freed individually, only the entire arena can be reset
pub struct BumpAllocator {
    start: *mut u8,
    current: *mut u8,
    end: *mut u8,
    size: usize,
}

unsafe impl Send for BumpAllocator {}
unsafe impl Sync for BumpAllocator {}

impl BumpAllocator {
    /// Create a new bump allocator with the specified size
    pub fn new(size: usize) -> Result<Self, &'static str> {
        if size == 0 {
            return Err("Allocator size must be greater than 0");
        }

        let layout = Layout::from_size_align(size, 8).map_err(|_| "Invalid layout")?;

        unsafe {
            let ptr = alloc(layout);
            if ptr.is_null() {
                return Err("Failed to allocate memory");
            }

            Ok(Self {
                start: ptr,
                current: ptr,
                end: ptr.add(size),
                size,
            })
        }
    }

    /// Allocate memory of the specified size and alignment
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        let align_offset = self.current.align_offset(align);
        let aligned_ptr = unsafe { self.current.add(align_offset) };
        let new_current = unsafe { aligned_ptr.add(size) };

        if new_current > self.end {
            return None; // Out of memory
        }

        self.current = new_current;
        NonNull::new(aligned_ptr)
    }

    /// Reset the allocator, freeing all allocations
    pub fn reset(&mut self) {
        self.current = self.start;
    }

    /// Get the number of bytes currently allocated
    pub fn used(&self) -> usize {
        unsafe { self.current.offset_from(self.start) as usize }
    }

    /// Get the total capacity
    pub fn capacity(&self) -> usize {
        self.size
    }
}

impl Drop for BumpAllocator {
    fn drop(&mut self) {
        unsafe {
            let layout = Layout::from_size_align(self.size, 8).unwrap();
            dealloc(self.start, layout);
        }
    }
}

/// Arena allocator - manages multiple bump allocators
pub struct Arena {
    allocators: Vec<BumpAllocator>,
    current_allocator: usize,
    allocator_size: usize,
}

impl Arena {
    /// Create a new arena with the specified allocator size
    pub fn new(allocator_size: usize) -> Result<Self, &'static str> {
        let first_allocator = BumpAllocator::new(allocator_size)?;
        Ok(Self {
            allocators: vec![first_allocator],
            current_allocator: 0,
            allocator_size,
        })
    }

    /// Allocate memory, creating a new allocator if needed
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        // Try current allocator
        if let Some(ptr) = self.allocators[self.current_allocator].allocate(size, align) {
            return Some(ptr);
        }

        // Create new allocator if current is full
        match BumpAllocator::new(self.allocator_size.max(size * 2)) {
            Ok(mut new_allocator) => {
                if let Some(ptr) = new_allocator.allocate(size, align) {
                    self.allocators.push(new_allocator);
                    self.current_allocator = self.allocators.len() - 1;
                    Some(ptr)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// Reset all allocators
    pub fn reset(&mut self) {
        for allocator in &mut self.allocators {
            allocator.reset();
        }
        self.current_allocator = 0;
    }

    /// Get total memory used across all allocators
    pub fn total_used(&self) -> usize {
        self.allocators.iter().map(|a| a.used()).sum()
    }

    /// Get total capacity across all allocators
    pub fn total_capacity(&self) -> usize {
        self.allocators.iter().map(|a| a.capacity()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_allocator() {
        let mut allocator = BumpAllocator::new(1024).unwrap();

        let _ptr1 = allocator.allocate(16, 8).unwrap();
        assert_eq!(allocator.used(), 16);

        let _ptr2 = allocator.allocate(32, 8).unwrap();
        assert_eq!(allocator.used(), 48);

        allocator.reset();
        assert_eq!(allocator.used(), 0);
    }

    #[test]
    fn test_arena() {
        let mut arena = Arena::new(256).unwrap();

        let _ptr1 = arena.allocate(64, 8).unwrap();
        let _ptr2 = arena.allocate(128, 8).unwrap();

        assert!(arena.total_used() > 0);

        arena.reset();
        assert_eq!(arena.total_used(), 0);
    }
}
