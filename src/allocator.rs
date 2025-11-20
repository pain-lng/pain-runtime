// Basic allocator module - bump allocator and arena allocator with optimizations

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
    /// Optimized for common alignment values (8, 16, 32, 64)
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        // Optimize alignment calculation for power-of-2 alignments
        let align_offset = if align.is_power_of_two() {
            // Fast path: use bitwise operations for power-of-2 alignment
            let mask = align - 1;
            let current_addr = self.current as usize;
            let aligned_addr = (current_addr + mask) & !mask;
            aligned_addr - current_addr
        } else {
            self.current.align_offset(align)
        };

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

/// Memory pool for fixed-size allocations
pub struct MemoryPool {
    block_size: usize,
    blocks: Vec<*mut u8>,
    free_list: Vec<*mut u8>,
    pool_size: usize,
}

impl MemoryPool {
    /// Create a new memory pool with specified block size and capacity
    pub fn new(block_size: usize, capacity: usize) -> Result<Self, &'static str> {
        if block_size == 0 || capacity == 0 {
            return Err("Block size and capacity must be greater than 0");
        }

        // Align block size to next power of 2 for better performance
        let aligned_block_size = block_size.next_power_of_two();
        let pool_size = aligned_block_size * capacity;

        let layout =
            Layout::from_size_align(pool_size, aligned_block_size).map_err(|_| "Invalid layout")?;

        unsafe {
            let ptr = alloc(layout);
            if ptr.is_null() {
                return Err("Failed to allocate memory pool");
            }

            let mut blocks = Vec::with_capacity(capacity);
            let mut free_list = Vec::with_capacity(capacity);

            // Initialize free list
            for i in 0..capacity {
                let block_ptr = ptr.add(i * aligned_block_size);
                blocks.push(block_ptr);
                free_list.push(block_ptr);
            }

            Ok(Self {
                block_size: aligned_block_size,
                blocks,
                free_list,
                pool_size,
            })
        }
    }

    /// Allocate a block from the pool
    pub fn allocate(&mut self) -> Option<NonNull<u8>> {
        self.free_list.pop().and_then(NonNull::new)
    }

    /// Deallocate a block back to the pool
    pub fn deallocate(&mut self, ptr: NonNull<u8>) {
        // Verify pointer is in pool range
        let ptr_addr = ptr.as_ptr() as usize;
        let pool_start = self.blocks[0] as usize;
        let pool_end = pool_start + self.pool_size;

        if ptr_addr >= pool_start && ptr_addr < pool_end {
            // Check alignment
            if (ptr_addr - pool_start).is_multiple_of(self.block_size) {
                self.free_list.push(ptr.as_ptr());
            }
        }
    }

    /// Get number of free blocks
    pub fn free_count(&self) -> usize {
        self.free_list.len()
    }

    /// Get number of allocated blocks
    pub fn allocated_count(&self) -> usize {
        self.blocks.len() - self.free_list.len()
    }
}

impl Drop for MemoryPool {
    fn drop(&mut self) {
        if !self.blocks.is_empty() {
            unsafe {
                let layout = Layout::from_size_align(self.pool_size, self.block_size).unwrap();
                dealloc(self.blocks[0], layout);
            }
        }
    }
}

/// Arena allocator - manages multiple bump allocators with memory pools
pub struct Arena {
    allocators: Vec<BumpAllocator>,
    current_allocator: usize,
    allocator_size: usize,
    pools: Vec<MemoryPool>, // Memory pools for common sizes
}

impl Arena {
    /// Create a new arena with the specified allocator size
    pub fn new(allocator_size: usize) -> Result<Self, &'static str> {
        let first_allocator = BumpAllocator::new(allocator_size)?;

        // Create memory pools for common sizes (8, 16, 32, 64, 128 bytes)
        let mut pools = Vec::new();
        for &size in &[8, 16, 32, 64, 128] {
            if let Ok(pool) = MemoryPool::new(size, 256) {
                pools.push(pool);
            }
        }

        Ok(Self {
            allocators: vec![first_allocator],
            current_allocator: 0,
            allocator_size,
            pools,
        })
    }

    /// Allocate memory, using pools for common sizes, creating a new allocator if needed
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        // Try memory pools for common sizes
        for pool in &mut self.pools {
            if pool.block_size >= size && pool.block_size % align == 0 {
                if let Some(ptr) = pool.allocate() {
                    return Some(ptr);
                }
            }
        }

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

    /// Deallocate memory (returns to pool if applicable)
    pub fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) {
        // Try to return to appropriate pool
        for pool in &mut self.pools {
            if pool.block_size == size {
                pool.deallocate(ptr);
                return;
            }
        }
        // For non-pool allocations, we don't deallocate (bump allocator behavior)
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
