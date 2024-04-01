use once_cell::sync::Lazy;
use std::ffi::c_void;

pub type NextFn = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);

static SYSTEM_THREAD_ATEXIT: Lazy<Option<NextFn>> = Lazy::new(|| unsafe {
    #[allow(clippy::transmute_ptr_to_ref)]
    let name = c"__cxa_thread_atexit_impl".as_ptr();
    std::mem::transmute(libc::dlsym(
        libc::RTLD_NEXT,
        #[allow(clippy::transmute_ptr_to_ref)]
        name,
    ))
});

/// Turns glibc's TLS destructor register function, `__cxa_thread_atexit_impl`,
/// into a no-op if hot reloading is enabled.
///
/// # Safety
/// This needs to be public for symbol visibility reasons, but you should
/// never need to call this yourself
pub unsafe fn thread_atexit(func: *mut c_void, obj: *mut c_void, dso_symbol: *mut c_void) {
    if crate::is_hot_reload_enabled() {
        // avoid registering TLS destructors on purpose, to avoid
        // double-frees and general crashiness
    } else if let Some(system_thread_atexit) = *SYSTEM_THREAD_ATEXIT {
        // hot reloading is disabled, and system provides `__cxa_thread_atexit_impl`,
        // so forward the call to it.
        system_thread_atexit(func, obj, dso_symbol);
    } else {
        // hot reloading is disabled *and* we don't have `__cxa_thread_atexit_impl`,
        // throw hands up in the air and leak memory.
    }
}

#[macro_export]
macro_rules! register_reload {
    () => {
        #[cfg(target_os = "linux")]
        #[no_mangle]
        pub unsafe extern "C" fn __cxa_thread_atexit_impl(
            func: *mut std::ffi::c_void,
            obj: *mut std::ffi::c_void,
            dso_symbol: *mut std::ffi::c_void,
        ) {
            $crate::hack::thread_atexit(func, obj, dso_symbol);
        }
    };
}
