#include "ffi/FieldValue.h"

namespace esotereel {

FieldValue::FieldValue() : m_kind(Kind::String), m_value() {}

FieldValue::FieldValue(const QVariant &variant, Kind kind) : m_kind(kind), m_value(variant) {}

FieldValue::FieldValue(const FieldValue &other) : m_kind(other.m_kind), m_value(other.m_value) {}

FieldValue::FieldValue(FieldValue &&other) noexcept
    : m_kind(other.m_kind), m_value(std::move(other.m_value)) {}

FieldValue::~FieldValue() = default;

FieldValue &FieldValue::operator=(const FieldValue &other) {
    if (this != &other) {
        m_kind = other.m_kind;
        m_value = other.m_value;
    }
    return *this;
}

FieldValue &FieldValue::operator=(FieldValue &&other) noexcept {
    if (this != &other) {
        m_kind = other.m_kind;
        m_value = std::move(other.m_value);
    }
    return *this;
}

FieldValue FieldValue::fromBool(bool value) {
    FieldValue fv;
    fv.m_kind = Kind::Bool;
    fv.m_value = QVariant::fromValue(value);
    return fv;
}

FieldValue FieldValue::fromInt(int64_t value) {
    FieldValue fv;
    fv.m_kind = Kind::Int;
    fv.m_value = QVariant::fromValue(value);
    return fv;
}

FieldValue FieldValue::fromFloat(double value) {
    FieldValue fv;
    fv.m_kind = Kind::Float;
    fv.m_value = QVariant::fromValue(value);
    return fv;
}

FieldValue FieldValue::fromString(const QString &value) {
    FieldValue fv;
    fv.m_kind = Kind::String;
    fv.m_value = QVariant::fromValue(value);
    return fv;
}

FieldValue FieldValue::fromEnum(const QString &value) {
    FieldValue fv;
    fv.m_kind = Kind::Enum;
    fv.m_value = QVariant::fromValue(value);
    return fv;
}

FieldValue FieldValue::fromColor(const QColor &color) {
    FieldValue fv;
    fv.m_kind = Kind::Color;
    fv.m_value = QVariant::fromValue(color);
    return fv;
}

FieldValue FieldValue::fromPaths(const QStringList &paths) {
    FieldValue fv;
    fv.m_kind = Kind::Path;
    fv.m_value = QVariant::fromValue(paths);
    return fv;
}

FieldValue FieldValue::fromArray(const QVariantList &items) {
    FieldValue fv;
    fv.m_kind = Kind::Array;
    fv.m_value = QVariant::fromValue(items);
    return fv;
}

FieldValue FieldValue::fromMap(const QVariantMap &map) {
    FieldValue fv;
    fv.m_kind = Kind::Map;
    fv.m_value = QVariant::fromValue(map);
    return fv;
}

FieldValue FieldValue::fromVariant(const QVariant &variant) {
    FieldValue fv;
    fv.m_value = variant;

    switch (variant.userType()) {
    case QMetaType::Bool:
        fv.m_kind = Kind::Bool;
        break;
    case QMetaType::Int:
    case QMetaType::LongLong:
    case QMetaType::UInt:
    case QMetaType::ULongLong:
        fv.m_kind = Kind::Int;
        break;
    case QMetaType::Double:
    case QMetaType::Float:
        fv.m_kind = Kind::Float;
        break;
    case QMetaType::QColor:
        fv.m_kind = Kind::Color;
        break;
    case QMetaType::QStringList:
        fv.m_kind = Kind::Path;
        break;
    case QMetaType::QVariantList:
        fv.m_kind = Kind::Array;
        break;
    case QMetaType::QVariantMap:
        fv.m_kind = Kind::Map;
        break;
    default:
        fv.m_kind = Kind::String;
        break;
    }

    return fv;
}

QVariant FieldValue::toVariant() const {
    return m_value;
}

FieldValue::Kind FieldValue::kind() const noexcept {
    return m_kind;
}

bool FieldValue::asBool(bool defaultValue) const {
    return m_value.canConvert<bool>() ? m_value.toBool() : defaultValue;
}

int64_t FieldValue::asInt(int64_t defaultValue) const {
    return m_value.canConvert<int64_t>() ? m_value.toLongLong() : defaultValue;
}

double FieldValue::asFloat(double defaultValue) const {
    return m_value.canConvert<double>() ? m_value.toDouble() : defaultValue;
}

QString FieldValue::asString(const QString &defaultValue) const {
    return m_value.canConvert<QString>() ? m_value.toString() : defaultValue;
}

QColor FieldValue::asColor(const QColor &defaultValue) const {
    return m_value.canConvert<QColor>() ? m_value.value<QColor>() : defaultValue;
}

QStringList FieldValue::asPaths() const {
    if (m_value.canConvert<QStringList>()) {
        return m_value.toStringList();
    }
    return {};
}

QVariantList FieldValue::asArray() const {
    if (m_value.canConvert<QVariantList>()) {
        return m_value.toList();
    }
    return {};
}

QVariantMap FieldValue::asMap() const {
    if (m_value.canConvert<QVariantMap>()) {
        return m_value.toMap();
    }
    return {};
}

bool FieldValue::isValid() const noexcept {
    return m_value.isValid();
}

} // namespace esotereel
