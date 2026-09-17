use std::any::Any;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::{ffi::state::ClientStateHandle, state::ClientState};

use cxx_qt_lib::{QColor, QMetaTypeType, QVariant, QVariantValue};
use esotereel_lib::plugin::property::value::FieldValue;
use esotereel_lib::util::color::RgbaColor;

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qcolor.h");
        type QColor = cxx_qt_lib::QColor;

        include!("cxx-qt-lib/qlist.h");
        // Instantiate QList container bindings inside the CXX bridge
        type QList_QVariant = cxx_qt_lib::QList<QVariant>;
        type QList_QString = cxx_qt_lib::QList<QString>;

        include!("cxx-qt-lib/qmap.h");
        // Instantiate QMap / QVariantMap binding
        type QVariantMap = cxx_qt_lib::QVariantMap;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum QFieldValueKind {
        Bool,
        Int,
        Float,
        Enum,
        String,
        Path,
        Color,
        Array,
        Map,
    }

    extern "Rust" {
        type ClientStateHandle;
        type CxxQtFieldValue;

        // Constructors
        fn cxxqt_field_value_new_bool(b: bool) -> Box<CxxQtFieldValue>;
        fn cxxqt_field_value_new_int(i: i64) -> Box<CxxQtFieldValue>;
        fn cxxqt_field_value_new_float(f: f64) -> Box<CxxQtFieldValue>;
        fn cxxqt_field_value_new_string(s: &QString) -> Box<CxxQtFieldValue>;
        fn cxxqt_field_value_new_enum(s: &QString) -> Box<CxxQtFieldValue>;
        fn cxxqt_field_value_new_color(c: &QColor) -> Box<CxxQtFieldValue>;
        fn cxxqt_field_value_from_qvariant(variant: &QVariant) -> Box<CxxQtFieldValue>;

        // Conversion to QVariant
        fn to_qvariant(self: &CxxQtFieldValue) -> QVariant;
        fn kind(self: &CxxQtFieldValue) -> QFieldValueKind;

        // Direct getters
        fn as_bool(self: &CxxQtFieldValue) -> bool;
        fn as_int(self: &CxxQtFieldValue) -> i64;
        fn as_float(self: &CxxQtFieldValue) -> f64;
        fn as_string(self: &CxxQtFieldValue) -> QString;
        fn as_color(self: &CxxQtFieldValue) -> QColor;
        fn as_array(self: &CxxQtFieldValue) -> QList_QVariant;
        fn as_map(self: &CxxQtFieldValue) -> QVariantMap;
        fn as_paths(self: &CxxQtFieldValue) -> QList_QString;

        // Utility functions for direct conversion
        fn field_value_to_variant(value: &CxxQtFieldValue) -> QVariant;
        fn variant_to_field_value(variant: &QVariant) -> Box<CxxQtFieldValue>;

        // Settings access using CXX-Qt
        unsafe fn cxxqt_settings_get_value(
            ptr_state: *const ClientStateHandle,
            key: &QString,
        ) -> QVariant;

        unsafe fn cxxqt_settings_set_value(
            ptr_state: *const ClientStateHandle,
            key: &QString,
            value: &QVariant,
        ) -> bool;
    }
}

use ffi::{QList_QString, QList_QVariant, QString, QVariantMap};

pub struct CxxQtFieldValue {
    pub inner: FieldValue,
}

impl CxxQtFieldValue {
    pub fn new(inner: FieldValue) -> Self {
        Self { inner }
    }

    pub fn kind(&self) -> ffi::QFieldValueKind {
        match &self.inner {
            FieldValue::Bool(_) => ffi::QFieldValueKind::Bool,
            FieldValue::Int(_) => ffi::QFieldValueKind::Int,
            FieldValue::Float(_) => ffi::QFieldValueKind::Float,
            FieldValue::Enum(_) => ffi::QFieldValueKind::Enum,
            FieldValue::String(_) => ffi::QFieldValueKind::String,
            FieldValue::Path(_) => ffi::QFieldValueKind::Path,
            FieldValue::Color(_) => ffi::QFieldValueKind::Color,
            FieldValue::Array(_) => ffi::QFieldValueKind::Array,
            FieldValue::Map(_) => ffi::QFieldValueKind::Map,
        }
    }

    pub fn to_qvariant(&self) -> QVariant {
        field_value_to_qvariant(&self.inner)
    }

    pub fn as_bool(&self) -> bool {
        match &self.inner {
            FieldValue::Bool(b) => *b,
            _ => false,
        }
    }

    pub fn as_int(&self) -> i64 {
        match &self.inner {
            FieldValue::Int(i) => *i,
            _ => 0,
        }
    }

    pub fn as_float(&self) -> f64 {
        match &self.inner {
            FieldValue::Float(f) => *f,
            _ => 0.0,
        }
    }

    pub fn as_string(&self) -> QString {
        match &self.inner {
            FieldValue::String(s) | FieldValue::Enum(s) => QString::from(s.as_str()),
            _ => QString::default(),
        }
    }

    pub fn as_color(&self) -> QColor {
        match &self.inner {
            FieldValue::Color(c) => {
                QColor::from_rgba(c.r as i32, c.g as i32, c.b as i32, c.a as i32)
            }
            _ => QColor::default(),
        }
    }

    pub fn as_array(&self) -> QList_QVariant {
        match &self.inner {
            FieldValue::Array(arr) => {
                let mut list = QList_QVariant::default();
                for item in arr {
                    list.append(field_value_to_qvariant(item));
                }
                list
            }
            _ => QList_QVariant::default(),
        }
    }

    pub fn as_map(&self) -> QVariantMap {
        match &self.inner {
            FieldValue::Map(map) => {
                let mut qmap = QVariantMap::default();
                for (k, v) in map {
                    qmap.insert(QString::from(k.as_str()), field_value_to_qvariant(v));
                }
                qmap
            }
            _ => QVariantMap::default(),
        }
    }

    pub fn as_paths(&self) -> QList_QString {
        match &self.inner {
            FieldValue::Path(paths) => {
                let mut list = QList_QString::default();
                for p in paths {
                    list.append(QString::from(p.to_string_lossy().as_ref()));
                }
                list
            }
            _ => QList_QString::default(),
        }
    }
}

// CXX bridge functions
fn cxxqt_field_value_new_bool(b: bool) -> Box<CxxQtFieldValue> {
    Box::new(CxxQtFieldValue::new(FieldValue::Bool(b)))
}

fn cxxqt_field_value_new_int(i: i64) -> Box<CxxQtFieldValue> {
    Box::new(CxxQtFieldValue::new(FieldValue::Int(i)))
}

fn cxxqt_field_value_new_float(f: f64) -> Box<CxxQtFieldValue> {
    Box::new(CxxQtFieldValue::new(FieldValue::Float(f)))
}

fn cxxqt_field_value_new_string(s: &QString) -> Box<CxxQtFieldValue> {
    Box::new(CxxQtFieldValue::new(FieldValue::String(s.to_string())))
}

fn cxxqt_field_value_new_enum(s: &QString) -> Box<CxxQtFieldValue> {
    Box::new(CxxQtFieldValue::new(FieldValue::Enum(s.to_string())))
}

fn cxxqt_field_value_new_color(c: &QColor) -> Box<CxxQtFieldValue> {
    Box::new(CxxQtFieldValue::new(FieldValue::Color(RgbaColor {
        r: c.red() as u8,
        g: c.green() as u8,
        b: c.blue() as u8,
        a: c.alpha() as u8,
    })))
}

fn cxxqt_field_value_from_qvariant(variant: &QVariant) -> Box<CxxQtFieldValue> {
    Box::new(CxxQtFieldValue::new(qvariant_to_field_value(variant)))
}

fn field_value_to_variant(value: &CxxQtFieldValue) -> QVariant {
    value.to_qvariant()
}

fn variant_to_field_value(variant: &QVariant) -> Box<CxxQtFieldValue> {
    Box::new(CxxQtFieldValue::new(qvariant_to_field_value(variant)))
}

unsafe fn cxxqt_settings_get_value(ptr_state: *const ClientStateHandle, key: &QString) -> QVariant {
    if ptr_state.is_null() {
        return QVariant::default();
    }

    let raw_state = ptr_state as *const Mutex<ClientState>;
    let state: Arc<Mutex<ClientState>> = unsafe {
        Arc::increment_strong_count(raw_state);
        Arc::from_raw(raw_state)
    };

    let key_str = key.to_string();
    let Ok(namespaced_id) = esotereel_lib::plugin::NamespacedID::parse(&key_str) else {
        return QVariant::default();
    };

    let state = state.lock().expect("mutex poisoned");
    match state.settings.get_value(&namespaced_id) {
        Some(val) => field_value_to_qvariant(val),
        None => QVariant::default(),
    }
}

unsafe fn cxxqt_settings_set_value(
    ptr_state: *const ClientStateHandle,
    key: &QString,
    value: &QVariant,
) -> bool {
    if ptr_state.is_null() {
        return false;
    }

    let raw_state = ptr_state as *const Mutex<ClientState>;
    let state: Arc<Mutex<ClientState>> = unsafe {
        Arc::increment_strong_count(raw_state);
        Arc::from_raw(raw_state)
    };

    let key_str = key.to_string();
    let Ok(namespaced_id) = esotereel_lib::plugin::NamespacedID::parse(&key_str) else {
        return false;
    };

    let field_value = qvariant_to_field_value(value);
    let mut state = state.lock().expect("mutex poisoned");
    state.settings.set_value(namespaced_id, field_value).is_ok()
}

/// Convert Rust FieldValue to Qt QVariant
pub fn field_value_to_qvariant(val: &FieldValue) -> QVariant {
    match val {
        FieldValue::Bool(b) => QVariant::from(b),
        FieldValue::Int(i) => QVariant::from(i),
        FieldValue::Float(f) => QVariant::from(f),
        FieldValue::Enum(s) | FieldValue::String(s) => {
            let qstr = QString::from(s.as_str());
            QVariant::from(&qstr)
        }
        FieldValue::Path(paths) => {
            let mut list = QList_QVariant::default();
            for p in paths {
                let path = QString::from(p.to_string_lossy().as_ref());
                list.append(QVariant::from(&path));
            }
            QVariant::from_qvariant_list(&list)
        }
        FieldValue::Color(c) => {
            let qcolor = QColor::from_rgba(c.r as i32, c.g as i32, c.b as i32, c.a as i32);
            QVariant::from(&qcolor)
        }
        FieldValue::Array(arr) => {
            let mut list = QList_QVariant::default();
            for item in arr {
                list.append(field_value_to_qvariant(item));
            }
            QVariant::from_qvariant_list(&list)
        }
        FieldValue::Map(map) => {
            let mut qmap = QVariantMap::default();
            for (k, v) in map {
                qmap.insert(QString::from(k.as_str()), field_value_to_qvariant(v));
            }
            QVariant::from(&qmap)
        }
    }
}

/// Convert Qt QVariant to Rust FieldValue
pub fn qvariant_to_field_value(variant: &QVariant) -> FieldValue {
    match variant.type_id() {
        t if t == QMetaTypeType::Bool => {
            FieldValue::Bool(variant.value::<bool>().unwrap_or_default())
        }
        t if t == QMetaTypeType::Int
            || t == QMetaTypeType::UInt
            || t == QMetaTypeType::LongLong
            || t == QMetaTypeType::ULongLong
            || t == QMetaTypeType::Short
            || t == QMetaTypeType::Long =>
        {
            FieldValue::Int(variant.value::<i64>().unwrap_or_default())
        }
        t if t == QMetaTypeType::Float || t == QMetaTypeType::Double => {
            FieldValue::Float(variant.value::<f64>().unwrap_or_default())
        }
        t if t == QMetaTypeType::QColor => {
            let qc = variant.value::<QColor>().unwrap_or_default();
            FieldValue::Color(RgbaColor {
                r: qc.red() as u8,
                g: qc.green() as u8,
                b: qc.blue() as u8,
                a: qc.alpha() as u8,
            })
        }
        // QStringList は Path 専用にしてある設計なので Array より先に判定する
        t if t == QMetaTypeType::QStringList => {
            let list = variant.value::<QList_QString>().unwrap_or_default();
            FieldValue::Path(list.iter().map(|s| PathBuf::from(s.to_string())).collect())
        }
        t if t == QMetaTypeType::QVariantList => {
            let list = variant.value::<QList_QVariant>().unwrap_or_default();
            FieldValue::Array(list.iter().map(qvariant_to_field_value).collect())
        }
        t if t == QMetaTypeType::QVariantMap => {
            let map = variant.value::<QVariantMap>().unwrap_or_default();
            FieldValue::Map(
                map.iter()
                    .map(|(k, v)| (k.to_string(), qvariant_to_field_value(v)))
                    .collect(),
            )
        }
        // QString、および未知の型はここに落ちる
        _ => FieldValue::String(variant.value::<QString>().unwrap_or_default().to_string()),
    }
}
