//! Test-only allocator probe; counts operations on the rendering test thread.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {static ENABLED:Cell<bool>=const{Cell::new(false)};static OPERATIONS:Cell<usize>=const{Cell::new(0)};}
struct GuardAllocator;
fn count() {
    if ENABLED.try_with(Cell::get).unwrap_or(false) {
        let _ = OPERATIONS.try_with(|n| n.set(n.get() + 1));
    }
}
// SAFETY: allocation and deallocation are delegated unchanged to the system allocator.
unsafe impl GlobalAlloc for GuardAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        count();
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(p, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: GuardAllocator = GuardAllocator;
pub fn begin() {
    OPERATIONS.set(0);
    ENABLED.set(true);
}
pub fn end() -> usize {
    ENABLED.set(false);
    OPERATIONS.get()
}
