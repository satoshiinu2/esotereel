use std::ffi::c_void;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};

pub struct SurfaceTarget {
    pub window: RawWindowHandle,
    pub display: RawDisplayHandle,
}

unsafe impl Send for SurfaceTarget {}
unsafe impl Sync for SurfaceTarget {}

impl HasWindowHandle for SurfaceTarget {
    fn window_handle(&'_ self) -> Result<WindowHandle<'_>, HandleError> {
        unsafe { Ok(WindowHandle::borrow_raw(self.window)) }
    }
}
impl HasDisplayHandle for SurfaceTarget {
    fn display_handle(&'_ self) -> Result<DisplayHandle<'_>, HandleError> {
        unsafe { Ok(DisplayHandle::borrow_raw(self.display)) }
    }
}

pub fn get_surface_target(
    window_ptr: *mut c_void,
    display_ptr: *mut c_void,
    is_wayland: bool,
) -> SurfaceTarget {
    // 1. Window Handle の作成
    let window_handle = {
        #[cfg(target_os = "windows")]
        {
            use std::num::NonZeroIsize;

            let mut h = raw_window_handle::Win32WindowHandle::new(
                NonZeroIsize::new(window_ptr as isize).expect("window_ptr is zero"),
            );

            // display_ptrを hinstance として使う
            h.hinstance = NonZeroIsize::new(display_ptr as isize);
            RawWindowHandle::Win32(h)
        }

        #[cfg(target_os = "linux")]
        {
            if is_wayland {
                use std::ptr::NonNull;

                let h = raw_window_handle::WaylandWindowHandle::new(
                    NonNull::new(window_ptr).expect("window_ptr is zero"),
                );
                RawWindowHandle::Wayland(h)
            } else {
                use std::num::NonZeroU32;

                let h = raw_window_handle::XcbWindowHandle::new(
                    NonZeroU32::new(window_ptr as u32).expect("window_ptr is zero"),
                );
                RawWindowHandle::Xcb(h)
            }
        }

        #[cfg(target_os = "macos")]
        {
            use std::ptr::NonNull;

            let h = raw_window_handle::AppKitWindowHandle::new(
                NonNull::new(window_ptr).expect("window_ptr is zero"),
            );
            RawWindowHandle::AppKit(h)
        }
    };

    // 2. Display Handle の作成 (Linuxでは必須、他は空でOK)
    let display_handle = {
        #[cfg(target_os = "linux")]
        {
            use std::ptr::NonNull;
            if is_wayland {
                let h = raw_window_handle::WaylandDisplayHandle::new(
                    NonNull::new(display_ptr).expect("display_ptr is zero"),
                );
                RawDisplayHandle::Wayland(h)
            } else {
                let h = raw_window_handle::XcbDisplayHandle::new(NonNull::new(display_ptr), 0);
                RawDisplayHandle::Xcb(h)
            }
        }

        #[cfg(target_os = "windows")]
        {
            RawDisplayHandle::Windows(raw_window_handle::WindowsDisplayHandle::new())
        }

        #[cfg(target_os = "macos")]
        {
            RawDisplayHandle::AppKit(raw_window_handle::AppKitDisplayHandle::new())
        }

        // それ以外（もしあれば）
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            RawDisplayHandle::UiKit(raw_window_handle::UiKitDisplayHandle::new())
        }
    };

    SurfaceTarget {
        window: window_handle,
        display: display_handle,
    }
}
