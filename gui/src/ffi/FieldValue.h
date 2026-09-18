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

    // Static factory methods
    static FieldValue fromBool(bool value) { return FieldValue(value); }
    static FieldValue fromInt(std::int64_t value) { return FieldValue(value); }
    static FieldValue fromFloat(double value) { return FieldValue(value); }
    static FieldValue fromEnum(const std::string &value) { return FieldValue(EnumValue{value}); }
    static FieldValue fromString(const std::string &value) { return FieldValue(value); }
    static FieldValue fromString(const QString &value) { return FieldValue(value.toStdString()); }
    static FieldValue fromPath(const PathValue &value) { return FieldValue(value); }
    static FieldValue fromColor(const RgbaColor &value) { return FieldValue(value); }
    static FieldValue fromArray(const Array &value) { return FieldValue(value); }
    static FieldValue fromMap(const Map &value) { return FieldValue(value); }

    const Variant &variant() const noexcept {
        return value_;
    }

    Variant &variant() noexcept {
        return value_;
    }

    // Type conversion methods
    bool asBool() const { return std::get<bool>(value_); }
    std::int64_t asInt() const { return std::get<std::int64_t>(value_); }
    double asFloat() const { return std::get<double>(value_); }
    std::string asEnum() const { return std::get<EnumValue>(value_).value; }
    std::string asString() const {
        if (std::holds_alternative<std::string>(value_)) {
            return std::get<std::string>(value_);
        } else if (std::holds_alternative<EnumValue>(value_)) {
            return std::get<EnumValue>(value_).value;
        }
        return "";
    }
    QString asQString() const { return QString::fromStdString(asString()); }
    PathValue asPath() const { return std::get<PathValue>(value_); }
    RgbaColor asColor() const { return std::get<RgbaColor>(value_); }
    Array asArray() const { return std::get<Array>(value_); }
    Map asMap() const { return std::get<Map>(value_); }

  private:
    Variant value_;
};
} // namespace esotereel
