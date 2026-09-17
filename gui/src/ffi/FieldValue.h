#pragma once

#include <QColor>
#include <QString>
#include <QStringList>
#include <QVariant>
#include <cstdint>

namespace esotereel {

class FieldValue {
  public:
    enum class Kind : uint8_t {
        Bool,
        Int,
        Float,
        Enum,
        String,
        Path,
        Color,
        Array,
        Map,
    };

    FieldValue();
    explicit FieldValue(const QVariant &variant, Kind kind = Kind::String);
    FieldValue(const FieldValue &other);
    FieldValue(FieldValue &&other) noexcept;
    ~FieldValue();

    FieldValue &operator=(const FieldValue &other);
    FieldValue &operator=(FieldValue &&other) noexcept;

    // Direct constructors
    static FieldValue fromBool(bool value);
    static FieldValue fromInt(int64_t value);
    static FieldValue fromFloat(double value);
    static FieldValue fromString(const QString &value);
    static FieldValue fromEnum(const QString &value);
    static FieldValue fromColor(const QColor &color);
    static FieldValue fromPaths(const QStringList &paths);
    static FieldValue fromArray(const QVariantList &items);
    static FieldValue fromMap(const QVariantMap &map);

    // QVariant conversion
    static FieldValue fromVariant(const QVariant &variant);
    QVariant toVariant() const;

    // Getters
    Kind kind() const noexcept;
    bool asBool(bool defaultValue = false) const;
    int64_t asInt(int64_t defaultValue = 0) const;
    double asFloat(double defaultValue = 0.0) const;
    QString asString(const QString &defaultValue = QString()) const;
    QColor asColor(const QColor &defaultValue = Qt::white) const;
    QStringList asPaths() const;
    QVariantList asArray() const;
    QVariantMap asMap() const;

    bool isValid() const noexcept;

  private:
    Kind m_kind;
    QVariant m_value;
};

} // namespace esotereel
