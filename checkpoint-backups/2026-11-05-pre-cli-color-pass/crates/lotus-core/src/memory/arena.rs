//! Autonomous memory space management for OS-agnostic and bare-metal execution.
//!
//! Provides a self-contained bump and arena allocator operating over a contiguous
//! physical memory slice or pointer without relying on OS virtual memory or libc malloc.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::ptr::NonNull;

#[derive(Debug, PartialEq)]
pub enum MemoryError {
    OutOfMemory { requested: usize, available: usize },
    InvalidAlignment,
    NullPointer,
}

/// A contiguous, self-contained physical or virtual memory region.
/// Passed by an exokernel (like BareMetal OS), bootloader, or host substrate.
pub struct MemoryRegion {
    pub base: NonNull<u8>,
    pub size: usize,
}

impl MemoryRegion {
    /// Create a memory region from a raw pointer and size.
    ///
    /// # Safety
    /// The caller must ensure that `base` points to at least `size` bytes of valid, writable memory.
    pub unsafe fn from_raw(base: *mut u8, size: usize) -> Result<Self, MemoryError> {
        let ptr = NonNull::new(base).ok_or(MemoryError::NullPointer)?;
        Ok(Self { base: ptr, size })
    }

    /// Create a memory region borrowing a mutable byte slice.
    pub fn from_slice(slice: &mut [u8]) -> Self {
        let ptr = NonNull::new(slice.as_mut_ptr()).expect("Slice pointer cannot be null");
        Self {
            base: ptr,
            size: slice.len(),
        }
    }
}

/// A deterministic, self-contained Bump Allocator operating entirely within a `MemoryRegion`.
pub struct BumpArena {
    base: NonNull<u8>,
    capacity: usize,
    allocated: usize,
}

impl BumpArena {
    pub fn new(region: MemoryRegion) -> Self {
        Self {
            base: region.base,
            capacity: region.size,
            allocated: 0,
        }
    }

    /// Allocate `size` bytes aligned to `align` bytes.
    pub fn alloc(&mut self, size: usize, align: usize) -> Result<NonNull<u8>, MemoryError> {
        if !align.is_power_of_two() {
            return Err(MemoryError::InvalidAlignment);
        }

        let current_ptr = unsafe { self.base.as_ptr().add(self.allocated) as usize };
        let aligned_ptr = (current_ptr + align - 1) & !(align - 1);
        let padding = aligned_ptr - current_ptr;

        let total_needed = padding.checked_add(size).ok_or(MemoryError::OutOfMemory {
            requested: size,
            available: self.capacity.saturating_sub(self.allocated),
        })?;

        if self.allocated + total_needed > self.capacity {
            return Err(MemoryError::OutOfMemory {
                requested: size,
                available: self.capacity.saturating_sub(self.allocated),
            });
        }

        self.allocated += total_needed;
        NonNull::new(aligned_ptr as *mut u8).ok_or(MemoryError::NullPointer)
    }

    /// Reset the arena to zero allocations without releasing the underlying memory region.
    pub fn reset(&mut self) {
        self.allocated = 0;
    }

    pub fn used(&self) -> usize {
        self.allocated
    }

    pub fn available(&self) -> usize {
        self.capacity.saturating_sub(self.allocated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_arena_allocation_and_alignment() {
        let mut buffer = [0u8; 1024];
        let region = MemoryRegion::from_slice(&mut buffer);
        let mut arena = BumpArena::new(region);

        // Allocate 10 bytes with 8-byte alignment
        let p1 = arena.alloc(10, 8).expect("Allocation 1 failed");
        assert_eq!(p1.as_ptr() as usize % 8, 0);

        // Allocate 5 bytes with 16-byte alignment
        let p2 = arena.alloc(5, 16).expect("Allocation 2 failed");
        assert_eq!(p2.as_ptr() as usize % 16, 0);

        assert!(arena.used() > 15);
        assert!(arena.available() < 1024);

        // Reset
        arena.reset();
        assert_eq!(arena.used(), 0);
        assert_eq!(arena.available(), 1024);
    }

    #[test]
    fn test_bump_arena_out_of_memory() {
        let mut buffer = [0u8; 64];
        let region = MemoryRegion::from_slice(&mut buffer);
        let mut arena = BumpArena::new(region);

        let res = arena.alloc(128, 8);
        assert_eq!(
            res,
            Err(MemoryError::OutOfMemory {
                requested: 128,
                available: 64
            })
        );
    }
}
