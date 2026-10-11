#pragma once

#include "esotereel_gui_helper.h"
#include <QColor>
#include <QString>
#include <QStringList>
#include <QVariant>
#include <cstdint>

namespace esotereel {

using CFieldValue = esotereel_gui_helper::CFieldValue;
using CFieldValueTag = esotereel_gui_helper::CFieldValueTag;

struct EnumValue {
    std::string value;
};

struct PathValue {
    std::vector<std::string> value;
};

struct RgbaColor {
    float r;
    float g;
    float b;
    float a;
};

class FieldValue {
  public:
    using Array = std::vector<FieldValue>;
    using Map = std::map<std::string, FieldValue>;
    using Path = PathValue;

    using Variant = std::variant<bool, std::int64_t, double, EnumValue, std::string, Path, RgbaColor, Array, Map>;

    FieldValue() = default;

    explicit FieldValue(Variant value) : value_(std::move(value)) {}

    static FieldValue fromC(const CFieldValue &value);

    // C++ -> Rust方向の変換。
    // 戻り値のCFieldValueは、String/Enum/Path/Mapの場合Path/Array/Mapの
    // バッファをこの関数内でnew[]する。使い終わったら必ずFieldValue::freeC()
    // で解放すること(Rust側のwrap_ffiが作るCFieldValueとは解放方法が異なる)。
    CFieldValue toC() const;

    // toC()で作ったCFieldValueを解放する。Rustが作ったCFieldValue
    // (esotereel_field_value_free等)とは別物なので混同しないこと。
    static void freeC(CFieldValue &value);

    // Static factory methods
    static FieldValue fromBool(bool value) {
        return FieldValue(value);
    }
    static FieldValue fromInt(std::int64_t value) {
        return FieldValue(value);
    }
    static FieldValue fromFloat(double value) {
        return FieldValue(value);
    }
    static FieldValue fromEnum(const std::string &value) {
        return FieldValue(EnumValue{value});
    }
    static FieldValue fromString(const std::string &value) {
        return FieldValue(value);
    }
    static FieldValue fromString(const QString &value) {
        return FieldValue(value.toStdString());
    }
    static FieldValue fromPath(const PathValue &value) {
        return FieldValue(value);
    }
    static FieldValue fromColor(const RgbaColor &value) {
        return FieldValue(value);
    }
    static FieldValue fromArray(const Array &value) {
        return FieldValue(value);
    }
    static FieldValue fromMap(const Map &value) {
        return FieldValue(value);
    }

    const Variant &variant() const noexcept {
        return value_;
    }

    Variant &variant() noexcept {
        return value_;
    }

    // Type conversion methods
    std::optional<bool> asBool() const {
        if (const auto *v = std::get_if<bool>(&value_)) {
            return *v;
        }
        return std::nullopt;
    }

    std::optional<std::int64_t> asInt() const {
        if (const auto *v = std::get_if<std::int64_t>(&value_)) {
            return *v;
        }
        return std::nullopt;
    }

    std::optional<double> asFloat() const {
        if (const auto *v = std::get_if<double>(&value_)) {
            return *v;
        }
        return std::nullopt;
    }

    std::optional<std::string> asEnum() const {
        if (const auto *v = std::get_if<EnumValue>(&value_)) {
            return v->value;
        }
        return std::nullopt;
    }

    std::optional<std::string> asString() const {
        if (const auto *v = std::get_if<std::string>(&value_)) {
            return *v;
        }
        if (const auto *v = std::get_if<EnumValue>(&value_)) {
            return v->value;
        }
        return std::nullopt;
    }

    std::optional<QString> asQString() const {
        if (auto v = asString()) {
            return QString::fromStdString(*v);
        }
        return std::nullopt;
    }

    std::optional<PathValue> asPath() const {
        if (const auto *v = std::get_if<PathValue>(&value_)) {
            return *v;
        }
        return std::nullopt;
    }

    std::optional<RgbaColor> asColor() const {
        if (const auto *v = std::get_if<RgbaColor>(&value_)) {
            return *v;
        }
        return std::nullopt;
    }

    std::optional<Array> asArray() const {
        if (const auto *v = std::get_if<Array>(&value_)) {
            return *v;
        }
        return std::nullopt;
    }

    std::optional<Map> asMap() const {
        if (const auto *v = std::get_if<Map>(&value_)) {
            return *v;
        }
        return std::nullopt;
    }

  private:
    Variant value_;
};
} // namespace esotereel

Q_DECLARE_METATYPE(esotereel::FieldValue)
