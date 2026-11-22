//! Shared Exokernel Substrate and Memory-Map Contract.
//!
//! Provides the memory window mapping and zero-copy shared mailbox interface
//! for running alongside `ggml` under BareMetal OS (Return Infinity) or custom unikernels.
//!
//! Layout in contiguous 64-bit physical address space:
//! ```text
//! +-------------------------------------------------------------------+
//! | Substrate Descriptor (Magic, ABI Version, Memory Boundaries)      |
//! +-------------------------------------------------------------------+
//! | Lotus 5-7-11 Memory Window (Workspace, DAG, BASIC, Arena)         |
//! +-------------------------------------------------------------------+
//! | Shared Parameter / Tensor Mailbox Window (Zero-Copy Transfer)    |
//! +-------------------------------------------------------------------+
//! | ggml Unikernel Memory Window (Model Weights, KV Cache, Compute)   |
//! +-------------------------------------------------------------------+
//! ```

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::ptr::NonNull;

pub const SUBSTRATE_MAGIC: [u8; 8] = *b"LOTUSEXO";
pub const SUBSTRATE_VERSION: u16 = 1;

/// Memory region descriptor for an individual execution partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Partition {
    pub offset: usize,
    pub size: usize,
}

/// The Canonical Substrate Descriptor mapped at the base of the shared memory space.
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C)]
pub struct SubstrateDescriptor {
    pub magic: [u8; 8],
    pub version: u16,
    pub flags: u16,
    pub total_memory: usize,
    pub lotus_partition: Partition,
    pub mailbox_partition: Partition,
    pub ggml_partition: Partition,
}

impl SubstrateDescriptor {
    /// Create a validated memory layout across total available physical memory.
    pub fn new_layout(total_memory: usize, lotus_size: usize, mailbox_size: usize) -> Result<Self, &'static str> {
        let desc_size = 64; // Reserved descriptor header size
        let needed = desc_size + lotus_size + mailbox_size;
        if total_memory < needed {
            return Err("Insufficient memory for substrate partitioning");
        }

        let lotus_partition = Partition {
            offset: desc_size,
            size: lotus_size,
        };

        let mailbox_partition = Partition {
            offset: desc_size + lotus_size,
            size: mailbox_size,
        };

        let ggml_offset = desc_size + lotus_size + mailbox_size;
        let ggml_size = total_memory - ggml_offset;

        let ggml_partition = Partition {
            offset: ggml_offset,
            size: ggml_size,
        };

        Ok(Self {
            magic: SUBSTRATE_MAGIC,
            version: SUBSTRATE_VERSION,
            flags: 0,
            total_memory,
            lotus_partition,
            mailbox_partition,
            ggml_partition,
        })
    }
}

/// Zero-copy Mailbox for direct tensor and parameter handoff between Lotus and ggml.
pub struct SharedMailbox {
    buffer: NonNull<u8>,
    size: usize,
}

impl SharedMailbox {
    pub unsafe fn from_raw(base: *mut u8, size: usize) -> Option<Self> {
        let ptr = NonNull::new(base)?;
        Some(Self { buffer: ptr, size })
    }

    pub fn write_payload(&mut self, offset: usize, data: &[u8]) -> Result<(), &'static str> {
        if offset + data.len() > self.size {
            return Err("Mailbox overflow");
        }
        unsafe {
            let dest = self.buffer.as_ptr().add(offset);
            core::ptr::copy_nonoverlapping(data.as_ptr(), dest, data.len());
        }
        Ok(())
    }

    pub fn read_payload<'a>(&self, offset: usize, len: usize) -> Result<&'a [u8], &'static str> {
        if offset + len > self.size {
            return Err("Mailbox underflow / read out of bounds");
        }
        unsafe {
            let src = self.buffer.as_ptr().add(offset);
            Ok(core::slice::from_raw_parts(src, len))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substrate_layout_partitioning() {
        // Assume 64 MB total physical memory handed by BareMetal OS
        let total_ram = 64 * 1024 * 1024;
        let lotus_ram = 16 * 1024 * 1024; // 16 MB for Lotus
        let mailbox_ram = 2 * 1024 * 1024; // 2 MB mailbox

        let desc = SubstrateDescriptor::new_layout(total_ram, lotus_ram, mailbox_ram).unwrap();

        assert_eq!(desc.magic, SUBSTRATE_MAGIC);
        assert_eq!(desc.lotus_partition.size, 16 * 1024 * 1024);
        assert_eq!(desc.mailbox_partition.size, 2 * 1024 * 1024);
        // Remainder given to ggml unikernel for weights/compute
        assert_eq!(desc.ggml_partition.size, total_ram - 64 - lotus_ram - mailbox_ram);
    }

    #[test]
    fn test_shared_mailbox_zero_copy() {
        let mut buffer = [0u8; 128];
        let mut mailbox = unsafe { SharedMailbox::from_raw(buffer.as_mut_ptr(), buffer.len()).unwrap() };

        let prompt = b"Model Weights Matrix Row [0.12, 0.45, 0.78]";
        mailbox.write_payload(0, prompt).unwrap();

        let read_back = mailbox.read_payload(0, prompt.len()).unwrap();
        assert_eq!(read_back, prompt);
    }
}
