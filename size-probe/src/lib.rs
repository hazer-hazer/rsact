//! Shared no_std runtime glue for the size-probe binaries: a global heap
//! (rsact needs `alloc`) and the panic handler. Deliberately does NOT reference
//! rsact-ui, so `--bin reactive` doesn't drag the UI crate into its graph.

#![no_std]

extern crate alloc;

use core::mem::MaybeUninit;
// Force-link cortex-m so its `critical-section-single-core` impl (the
// `_critical_section_1_0_*` symbols embedded-alloc / portable-atomic need) is
// present — otherwise the unused crate is DCE'd and linking fails.
use cortex_m as _;
use embedded_alloc::LlffHeap as Heap;
// Registers `#[panic_handler]` for the linked binary.
use panic_halt as _;

#[global_allocator]
static HEAP: Heap = Heap::empty();

// These binaries are measured, never executed, so the heap is never actually
// touched — the buffer only exists so `alloc` links. Keep it small so it doesn't
// dominate the `.bss` metric (framework RAM is heap-resident, not `.bss`).
const HEAP_SIZE: usize = 1024;
static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] =
    [MaybeUninit::uninit(); HEAP_SIZE];

/// Initialize the heap. Call once at the top of `main` before any allocation.
pub fn init_heap() {
    unsafe {
        HEAP.init(core::ptr::addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE);
    }
}
