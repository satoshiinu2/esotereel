#include "FieldControlFactory.h"

#include <algorithm>
#include <QCheckBox>
#include <QColor>
#include <QColorDialog>
#include <QComboBox>
#include <QDialog>
#include <QDoubleSpinBox>
#include <QLineEdit>
#include <QHBoxLayout>
#include <QPushButton>
#include <QSpinBox>

namespace esotereel::window::widget {

using esotereel::FieldValue;
using SettingsFieldType = esotereel_gui_helper::SettingsFieldType;

QStringList FieldControlFactory::guessEnumOptions(const QString &key, const QString &currentValue) {
    QStringList options;
    if (key.contains("log_level") || key.contains("level")) {
        options << "Off" << "Error" << "Warn" << "Info" << "Debug" << "Trace";
    } else if (key.contains("theme")) {
        options << "Light" << "Dark" << "System";
    } else {
        options << currentValue;
    }
    return options;
}

QWidget *FieldControlFactory::createControl(const SettingsField &field, const FieldBinding &binding) {
    const FieldValue currentValue = binding.getValue();

    if (field.kindType == SettingsFieldType::Bool) {
        auto *checkBox = new QCheckBox();

        auto boolValue = currentValue.asBool();
        // Safe: field.kindType == SettingsFieldType::Bool guarantees the value type matches
        assert(boolValue.has_value());
        checkBox->setChecked(boolValue.value());

        QObject::connect(checkBox, &QCheckBox::checkStateChanged, checkBox, [binding](Qt::CheckState state) {
            binding.setValue(FieldValue::fromBool(state == Qt::Checked));
        });
        return checkBox;
    }

    if (field.kindType == SettingsFieldType::Int) {
        auto *spinBox = new QSpinBox();
        spinBox->setRange(0, 1000); // TODO: schemaにmin/maxが乗ったらそちらを使う

        auto intValue = currentValue.asInt();
        // Safe: field.kindType == SettingsFieldType::Int guarantees the value type matches
        assert(intValue.has_value());
        spinBox->setValue(intValue.value());

        QObject::connect(spinBox, QOverload<int>::of(&QSpinBox::valueChanged), spinBox,
                         [binding](int value) { binding.setValue(FieldValue::fromInt(value)); });
        return spinBox;
    }

    if (field.kindType == SettingsFieldType::Float) {
        auto *doubleSpinBox = new QDoubleSpinBox();
        doubleSpinBox->setRange(-1.0e9, 1.0e9); // TODO: use schema min/max when exposed by FFI

        auto floatValue = currentValue.asFloat();
        // Safe: field.kindType == SettingsFieldType::Float guarantees the value type matches
        assert(floatValue.has_value());
        doubleSpinBox->setValue(floatValue.value());

        QObject::connect(doubleSpinBox, QOverload<double>::of(&QDoubleSpinBox::valueChanged), doubleSpinBox,
                         [binding](double value) { binding.setValue(FieldValue::fromFloat(value)); });
        return doubleSpinBox;
    }

    if (field.kindType == SettingsFieldType::Enum) {
        auto *comboBox = new QComboBox();

        auto qstringValue = currentValue.asQString();
        // Safe: field.kindType == SettingsFieldType::Enum guarantees the value type matches
        assert(qstringValue.has_value());
        const QStringList options = guessEnumOptions(field.key, qstringValue.value());
        comboBox->addItems(options);
        comboBox->setCurrentText(qstringValue.value());

        QObject::connect(comboBox, &QComboBox::currentTextChanged, comboBox, [binding](const QString &text) {
            binding.setValue(FieldValue::fromEnum(text.toStdString()));
        });
        return comboBox;
    }

    if (field.kindType == SettingsFieldType::Color) {
        auto *colorButton = new QPushButton("Choose Color");
        QObject::connect(colorButton, &QPushButton::clicked, colorButton, [binding, colorButton]() {
            QColor color = Qt::white;
            const FieldValue existing = binding.getValue();
            const auto rgba = existing.asColor();
            // Safe: field.kindType == SettingsFieldType::Color guarantees the value type matches
            assert(rgba.has_value());
            color = QColor::fromRgbF(rgba->r, rgba->g, rgba->b, rgba->a);

            QColorDialog dialog(color, colorButton);
            if (dialog.exec() == QColorDialog::Accepted) {
                const QColor selected = dialog.selectedColor();
                RgbaColor out{static_cast<float>(selected.redF()), static_cast<float>(selected.greenF()),
                              static_cast<float>(selected.blueF()), static_cast<float>(selected.alphaF())};
                binding.setValue(FieldValue::fromColor(out));
            }
        });
        return colorButton;
    }

    if (field.kindType == SettingsFieldType::Array) {
        const auto values = currentValue.asArray();
        if (!values || !std::all_of(values->begin(), values->end(), [](const FieldValue &value) {
                return value.asFloat().has_value() || value.asInt().has_value();
            })) {
            auto *unsupported = new QLineEdit(QObject::tr("Unsupported array"));
            unsupported->setEnabled(false);
            return unsupported;
        }
        auto *container = new QWidget();
        auto *layout = new QHBoxLayout(container);
        layout->setContentsMargins(0, 0, 0, 0);
        for (size_t index = 0; index < values->size(); ++index) {
            auto *spinBox = new QDoubleSpinBox(container);
            spinBox->setRange(-1.0e9, 1.0e9);
            const auto &value = (*values)[index];
            const bool isInteger = value.asInt().has_value();
            spinBox->setDecimals(isInteger ? 0 : 3);
            spinBox->setValue(isInteger ? static_cast<double>(*value.asInt()) : *value.asFloat());
            layout->addWidget(spinBox);
            QObject::connect(spinBox, QOverload<double>::of(&QDoubleSpinBox::valueChanged), spinBox,
                             [binding, index, isInteger](double number) {
                                 auto latest = binding.getValue().asArray();
                                 if (!latest || index >= latest->size())
                                     return;
                                 (*latest)[index] = isInteger
                                     ? FieldValue::fromInt(static_cast<std::int64_t>(number))
                                     : FieldValue::fromFloat(number);
                                 binding.setValue(FieldValue::fromArray(*latest));
                             });
        }
        return container;
    }

    // String, および未対応種別(FilePath/Map)のフォールバック
    auto *lineEdit = new QLineEdit();
    auto qstringValue = currentValue.asQString();
    // Safe: fallback case assumes string type
    assert(qstringValue.has_value());
    lineEdit->setText(qstringValue.value());
    QObject::connect(lineEdit, &QLineEdit::textChanged, lineEdit,
                     [binding](const QString &text) { binding.setValue(FieldValue::fromString(text)); });
    return lineEdit;
}

QWidget *FieldControlFactory::createControl(const ClipPropertySchema &field, const FieldBinding &binding) {
    // ClipPropertySchema has the same structure as SettingsField, so we can reuse the same logic
    // by constructing a temporary SettingsField
    SettingsField tempField;
    tempField.key = field.key;
    tempField.category = field.category;
    tempField.label = field.label;
    tempField.kindType = field.kindType;
    tempField.defaultValue = field.defaultValue;
    return createControl(tempField, binding);
}

} // namespace esotereel::window::widget
