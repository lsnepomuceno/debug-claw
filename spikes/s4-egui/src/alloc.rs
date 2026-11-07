//! A counting wrapper around the system allocator, so the report can give
//! the live Rust heap next to the RSS. RSS also holds the GPU driver, the
//! font atlas and shared libraries, which hide what the views themselves
//! keep.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

pub struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            grow(layout.size());
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            grow(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Relaxed);
            grow(new_size);
        }
        p
    }
}

fn grow(n: usize) {
    let now = LIVE.fetch_add(n, Relaxed) + n;
    PEAK.fetch_max(now, Relaxed);
}

/// Bytes currently allocated through the Rust allocator.
pub fn live() -> usize {
    LIVE.load(Relaxed)
}

pub fn peak() -> usize {
    PEAK.load(Relaxed)
}

/// VmRSS of this process in bytes, read from /proc.
pub fn rss() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<usize>().ok())
        })
        .map_or(0, |kb| kb * 1024)
}

pub fn mb(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}
