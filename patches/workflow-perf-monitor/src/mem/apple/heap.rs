//! A wrapper around libmalloc APIs.
//!
//! On newer macOS (15+), `malloc_zone_t` is opaque and fields like
//! `zone_name` and `introspect` are no longer directly accessible.
//! We use `libc::malloc_zone_statistics` and `malloc_get_zone_name`
//! as safe alternatives.

use crate::bindings::{mach_task_self_, malloc_default_zone, malloc_zone_t, vm_address_t};
use std::{io, str};

/// Malloc zone statistics.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct MallocStatistics {
    pub blocks_in_use: u32,
    pub size_in_use: usize,
    pub max_size_in_use: usize,
    pub size_allocated: usize,
}

/// A Wrapper around `malloc_zone_t`, originally defined at `libmalloc.h`.
pub struct MallocZone(*mut malloc_zone_t);

extern "C" {
    fn malloc_get_zone_name(zone: *mut malloc_zone_t) -> *const std::ffi::c_char;
    fn malloc_zone_statistics(zone: *mut malloc_zone_t, stats: *mut MallocStatistics);
}

impl MallocZone {
    /// Get the name of this zone.
    pub fn name(&self) -> Result<&str, str::Utf8Error> {
        unsafe {
            let name_ptr = malloc_get_zone_name(self.0);
            if name_ptr.is_null() {
                return Ok("<unknown>");
            }
            std::ffi::CStr::from_ptr(name_ptr).to_str()
        }
    }
    /// Get the statistics of this zone.
    pub fn statistics(&mut self) -> Option<MallocStatistics> {
        unsafe {
            let mut stats = MallocStatistics::default();
            malloc_zone_statistics(self.0, &mut stats);
            Some(stats)
        }
    }
}

/// Get all malloc zones of current process.
///
/// # Safety
/// CAUTION: `MallocZone`s(*malloc_zone_t) returned by `malloc_get_all_zones`
/// may be destroyed by other threads.
pub unsafe fn malloc_get_all_zones() -> io::Result<Vec<MallocZone>> {
    let mut count: u32 = 0;
    let mut zones: *mut vm_address_t = std::ptr::null_mut();
    let ret =
        crate::bindings::malloc_get_all_zones(mach_task_self_, None, &mut zones, &mut count);
    if ret != 0 {
        Err(io::Error::from_raw_os_error(ret))
    } else {
        let zones =
            std::slice::from_raw_parts_mut(zones as *mut *mut malloc_zone_t, count as usize)
                .iter()
                .map(|&p| MallocZone(p))
                .collect::<Vec<_>>();
        Ok(zones)
    }
}

/// Get the default malloc zone of current process.
pub fn malloc_get_default_zone() -> MallocZone {
    MallocZone(unsafe { malloc_default_zone() })
}
