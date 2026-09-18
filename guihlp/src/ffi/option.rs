use crate::ffi::{field_value::CFieldValue, stringview::OwnedString};

#[repr(C)]
#[derive(Clone, Copy)]
pub union FfiOptionUnion<T: Copy> {
    pub some: T,
    pub none: (),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiOption<T: Copy> {
    pub has_value: bool,
    pub value: FfiOptionUnion<T>,
}

impl<T: Copy> From<Option<T>> for FfiOption<T> {
    fn from(opt: Option<T>) -> Self {
        FfiOption::from_option(opt)
    }
}

impl<T: Copy> FfiOption<T> {
    pub fn some(val: T) -> Self {
        Self {
            has_value: true,
            value: FfiOptionUnion { some: val },
        }
    }

    pub fn none() -> Self {
        Self {
            has_value: false,
            value: FfiOptionUnion { none: () },
        }
    }

    pub fn from_option(opt: Option<T>) -> Self {
        match opt {
            Some(v) => Self::some(v),
            None => Self::none(),
        }
    }
}

macro_rules! export_option {
    ($name:ident, $ty:ty) => {
        paste::paste! {
            #[allow(non_camel_case_types)]
            pub type [<FfiOption_ $name>] = FfiOption<$ty>;

            #[unsafe(no_mangle)]
            pub extern "C" fn [<ffi_option_ $name _free>](opt: *mut FfiOption<$ty>) {
                unsafe {
                    if (*opt).has_value {
                        std::ptr::drop_in_place(&mut (*opt).value.some);
                    }
                }
            }
        }
    };
}

// export_option!(i32, i32);
// export_option!(CFieldValue, CFieldValue);
// export_option!(String, OwnedString);
