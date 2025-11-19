// Garbage Collector for Pain runtime (dev profile)
// Simple mark-and-sweep GC implementation

use std::collections::{HashMap, HashSet};

/// GC-managed object header
#[derive(Debug)]
struct GcHeader {
    marked: bool,
    size: usize,
    // Future: type info, weak refs, etc.
}

/// GC-managed object
pub struct GcObject {
    header: *mut GcHeader,
    data: *mut u8,
}

unsafe impl Send for GcObject {}
unsafe impl Sync for GcObject {}

impl GcObject {
    /// Get pointer to data
    pub fn data_ptr(&self) -> *mut u8 {
        self.data
    }

    /// Get size of allocated data
    pub fn size(&self) -> usize {
        unsafe { (*self.header).size }
    }

    /// Mark this object as reachable
    pub fn mark(&self) {
        unsafe {
            (*self.header).marked = true;
        }
    }

    /// Check if object is marked
    pub fn is_marked(&self) -> bool {
        unsafe { (*self.header).marked }
    }
}

impl Drop for GcObject {
    fn drop(&mut self) {
        // GC will handle deallocation
    }
}

/// Garbage Collector - mark-and-sweep implementation
pub struct GarbageCollector {
    objects: HashMap<*mut u8, (GcHeader, usize)>, // data_ptr -> (header, size)
    roots: HashSet<*mut u8>,                      // Root pointers (variables, stack, etc.)
    total_allocated: usize,
    threshold: usize, // GC threshold in bytes
}

impl GarbageCollector {
    /// Create a new GC with default threshold (1MB)
    pub fn new() -> Self {
        Self::with_threshold(1024 * 1024)
    }

    /// Create a new GC with custom threshold
    pub fn with_threshold(threshold: usize) -> Self {
        Self {
            objects: HashMap::new(),
            roots: HashSet::new(),
            total_allocated: 0,
            threshold,
        }
    }

    /// Allocate a new GC-managed object
    pub fn allocate(&mut self, size: usize) -> Option<GcObject> {
        // Check if we need to run GC
        if self.total_allocated >= self.threshold {
            self.collect();
        }

        // Allocate header + data
        let header_size = std::mem::size_of::<GcHeader>();
        let align = 8;
        let total_size = header_size + size;
        let aligned_size = (total_size + align - 1) & !(align - 1);

        unsafe {
            let layout = std::alloc::Layout::from_size_align(aligned_size, align).ok()?;
            let ptr = std::alloc::alloc(layout);
            if ptr.is_null() {
                // Try GC and retry
                self.collect();
                let ptr = std::alloc::alloc(layout);
                if ptr.is_null() {
                    return None;
                }
            }

            // Initialize header
            let header_ptr = ptr as *mut GcHeader;
            (*header_ptr) = GcHeader {
                marked: false,
                size,
            };

            // Data starts after header
            let data_ptr = ptr.add(header_size);

            // Track object
            self.objects.insert(
                data_ptr,
                (
                    GcHeader {
                        marked: false,
                        size,
                    },
                    aligned_size,
                ),
            );
            self.total_allocated += aligned_size;

            Some(GcObject {
                header: header_ptr,
                data: data_ptr,
            })
        }
    }

    /// Register a root pointer (variable, stack reference, etc.)
    pub fn add_root(&mut self, ptr: *mut u8) {
        self.roots.insert(ptr);
    }

    /// Unregister a root pointer
    pub fn remove_root(&mut self, ptr: *mut u8) {
        self.roots.remove(&ptr);
    }

    /// Mark all reachable objects from roots
    fn mark_phase(&mut self) {
        // Reset all marks
        for (_, (header, _)) in self.objects.iter_mut() {
            header.marked = false;
        }

        // Mark all roots and recursively mark their references
        let mut to_mark: Vec<*mut u8> = self.roots.iter().copied().collect();
        let mut marked = HashSet::new();

        while let Some(ptr) = to_mark.pop() {
            if marked.contains(&ptr) {
                continue;
            }
            marked.insert(ptr);

            // Mark this object
            if let Some((header, _)) = self.objects.get_mut(&ptr) {
                header.marked = true;

                // For now, we don't traverse object internals
                // Future: scan object for pointers and add them to to_mark
            }
        }
    }

    /// Sweep phase - free unmarked objects
    fn sweep_phase(&mut self) {
        let mut to_remove = Vec::new();
        let mut freed = 0;

        for (data_ptr, (header, size)) in &self.objects {
            if !header.marked {
                to_remove.push(*data_ptr);
                freed += size;
            }
        }

        for data_ptr in to_remove {
            if let Some((_, total_size)) = self.objects.remove(&data_ptr) {
                unsafe {
                    // Calculate header size
                    let header_size = std::mem::size_of::<GcHeader>();
                    let align = 8;
                    let aligned_size = header_size + total_size;
                    let aligned_size = (aligned_size + align - 1) & !(align - 1);

                    // Get pointer to start of allocation (header)
                    let header_ptr = data_ptr.sub(header_size);

                    let layout = std::alloc::Layout::from_size_align(aligned_size, 8)
                        .expect("Invalid layout");
                    std::alloc::dealloc(header_ptr, layout);
                }
                self.total_allocated -= total_size;
            }
        }
    }

    /// Run garbage collection
    pub fn collect(&mut self) {
        self.mark_phase();
        self.sweep_phase();
    }

    /// Get memory statistics
    pub fn stats(&self) -> (usize, usize, usize) {
        let live_objects = self
            .objects
            .values()
            .filter(|(header, _)| header.marked)
            .count();
        (self.total_allocated, self.objects.len(), live_objects)
    }

    /// Force collection and return freed memory
    pub fn force_collect(&mut self) -> usize {
        let before = self.total_allocated;
        self.collect();
        before - self.total_allocated
    }
}

impl Default for GarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_basic() {
        let mut gc = GarbageCollector::with_threshold(1024);

        // Allocate some objects
        let obj1 = gc.allocate(64).unwrap();
        let obj2 = gc.allocate(128).unwrap();

        // Register as roots
        gc.add_root(obj1.data_ptr());
        gc.add_root(obj2.data_ptr());

        // Mark and sweep should keep both
        gc.collect();

        let (allocated, total, live) = gc.stats();
        assert!(allocated > 0);
        assert_eq!(total, 2);
        assert_eq!(live, 2);
    }

    #[test]
    fn test_gc_collect_unreachable() {
        let mut gc = GarbageCollector::with_threshold(1024);

        // Allocate objects
        let obj1 = gc.allocate(64).unwrap();
        let _obj2 = gc.allocate(128).unwrap();

        // Only register obj1 as root
        gc.add_root(obj1.data_ptr());

        // Collect should free obj2
        gc.collect();

        let (_, total, live) = gc.stats();
        assert_eq!(total, 1);
        assert_eq!(live, 1);
    }
}
