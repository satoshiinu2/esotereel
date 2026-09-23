#pragma once

#include "ffi/FieldValue.h"
#include "ffi/StringView.h"

namespace esotereel {
using CClipBindingValueTag = esotereel_gui_helper::CClipBindingValueTag;
using CClipBindingValue = esotereel_gui_helper::CClipBindingValue;

// SettingsFieldTypeと同じCの型を共有
using ClipPropertyFieldType = esotereel_gui_helper::SettingsFieldType;

class ClipPropertySchema {
  public:
    QString key;
    QString category;
    QString label;
    ClipPropertyFieldType kindType;
    FieldValue defaultValue;

    ClipPropertySchema() = default;

    ClipPropertySchema(esotereel_gui_helper::FfiPropertySchema ffi)
        : key(OwnedString::toQString(ffi.key)), category(OwnedString::toQString(ffi.category)),
          label(OwnedString::toQString(ffi.label)), kindType(ffi.kind_type),
          defaultValue(FieldValue::fromC(ffi.default_value)) {

        OwnedString::free(ffi.key);
        OwnedString::free(ffi.category);
        OwnedString::free(ffi.label);
    }
};

class ClipPropertyValue {
  public:
    static ClipPropertyValue fromStatic(const FieldValue &value) {
        ClipPropertyValue v;
        v.isStatic_ = true;
        v.staticValue = value;
        return v;
    }

    bool isStatic() const {
        return isStatic_;
    }
    const FieldValue &asStatic() const {
        return staticValue;
    }

    // TODO(Keyframes): isKeyframes() / keyframes() をここに追加し、
    // ClipPropertiesPanel側で「今どの表現かによって見せ方を変える」判断に使う。

    static ClipPropertyValue fromC(const CClipBindingValue &raw) {
        switch (raw.tag) {
        case CClipBindingValueTag::Static:
            return fromStatic(FieldValue::fromC(raw.data.static_value));
        // case CClipBindingValueTag::Keyframes: ...
        default:
            return fromStatic(FieldValue::fromBool(false)); // unreachable想定、要調整
        }
    }

    CClipBindingValue toC() const {
        CClipBindingValue out{};
        out.tag = CClipBindingValueTag::Static;
        out.data.static_value = staticValue.toC();
        return out;
    }

    static void freeC(CClipBindingValue &value) {
        if (value.tag == CClipBindingValueTag::Static) {
            FieldValue::freeC(value.data.static_value);
        }
    }

  private:
    bool isStatic_ = true;
    FieldValue staticValue;
};

} // namespace esotereel

Q_DECLARE_METATYPE(esotereel::ClipPropertySchema)
Q_DECLARE_METATYPE(esotereel::ClipPropertyValue)