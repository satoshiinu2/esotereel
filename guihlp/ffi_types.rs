pub const RESULT_TYPES: &[(&str, &str)] = &[
    ("i32", "i32"),
    ("f32", "f32"),
    ("bool", "bool"),
    ("void", "()"),
    ("CFieldValue", "CFieldValue"),
    ("TimelineId", "TimelineId"),
    ("ClipId", "ClipId"),
];

pub const RESULT_OPTION_TYPES: &[(&str, &str)] = &[("CFieldValue", "CFieldValue")];

pub const OPTION_TYPES: &[(&str, &str)] = &[("i32", "i32"), ("CFieldValue", "CFieldValue")];

pub const ARRAY_TYPES: &[(&str, &str)] = &[("i32", "i32"), ("f32", "f32"), ("CFieldValue", "CFieldValue")];
