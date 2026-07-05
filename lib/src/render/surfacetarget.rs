use std::{ffi::c_void, num::NonZeroIsize, num::NonZeroU32, ptr::NonNull};

use raw_window_handle::{
    AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
    Win32WindowHandle, WindowHandle, WindowsDisplayHandle, XcbDisplayHandle, XcbWindowHandle,
};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformKind {
    Unknown = 0,
    Xcb = 1,
    Wayland = 2,
    Win32 = 3,
    AppKit = 4,
}

impl PlatformKind {
    fn from_u32(v: u32) -> Self {
        match v {
            0 => PlatformKind::Unknown,
            1 => PlatformKind::Xcb,
            2 => PlatformKind::Wayland,
            3 => PlatformKind::Win32,
            4 => PlatformKind::AppKit,
            _ => PlatformKind::Unknown,
        }
    }
}

#[repr(C)]
pub struct NativeWindowHandle {
    pub kind: PlatformKind,
    pub window_ptr: *mut c_void,
    pub display_ptr: *mut c_void,
}

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

pub fn get_surface_target(handle: NativeWindowHandle) -> Result<SurfaceTarget, String> {
    let (window, display) = match handle.kind {
        PlatformKind::Xcb => {
            let window_id =
                NonZeroU32::new(handle.window_ptr as u32).ok_or("xcb window_ptr is zero")?;
            let w = XcbWindowHandle::new(window_id);

            let conn = NonNull::new(handle.display_ptr);
            let d = XcbDisplayHandle::new(conn, 0);

            (RawWindowHandle::Xcb(w), RawDisplayHandle::Xcb(d))
        }
        PlatformKind::Wayland => {
            let surface =
                NonNull::new(handle.window_ptr).ok_or("wayland window_ptr (wl_surface) is null")?;
            let w = WaylandWindowHandle::new(surface);

            let display = NonNull::new(handle.display_ptr)
                .ok_or("wayland display_ptr (wl_display) is null")?;
            let d = WaylandDisplayHandle::new(display);

            (RawWindowHandle::Wayland(w), RawDisplayHandle::Wayland(d))
        }
        PlatformKind::Win32 => {
            let hwnd = NonZeroIsize::new(handle.window_ptr as isize).ok_or("win32 hwnd is zero")?;
            let mut w = Win32WindowHandle::new(hwnd);
            w.hinstance = NonZeroIsize::new(handle.display_ptr as isize);

            (
                RawWindowHandle::Win32(w),
                RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
            )
        }
        PlatformKind::AppKit => {
            let ns_view = NonNull::new(handle.window_ptr).ok_or("appkit ns_view is null")?;
            let w = AppKitWindowHandle::new(ns_view);

            (
                RawWindowHandle::AppKit(w),
                RawDisplayHandle::AppKit(raw_window_handle::AppKitDisplayHandle::new()),
            )
        }
        PlatformKind::Unknown => return Err("unknown platform kind from C++ side".into()),
    };

    Ok(SurfaceTarget { window, display })
}
