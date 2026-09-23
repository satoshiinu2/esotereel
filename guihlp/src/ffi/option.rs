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
