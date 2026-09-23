#include "FieldControlFactory.h"

#include <QCheckBox>
#include <QColor>
#include <QColorDialog>
#include <QComboBox>
#include <QDialog>
#include <QDoubleSpinBox>
#include <QLineEdit>
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
        checkBox->setChecked(currentValue.asBool());
        QObject::connect(checkBox, &QCheckBox::checkStateChanged, checkBox, [binding](Qt::CheckState state) {
            binding.setValue(FieldValue::fromBool(state == Qt::Checked));
        });
        return checkBox;
    }

    if (field.kindType == SettingsFieldType::Int) {
        auto *spinBox = new QSpinBox();
        spinBox->setRange(0, 1000); // TODO: schemaにmin/maxが乗ったらそちらを使う
        spinBox->setValue(static_cast<int>(currentValue.asInt()));
        QObject::connect(spinBox, QOverload<int>::of(&QSpinBox::valueChanged), spinBox,
                         [binding](int value) { binding.setValue(FieldValue::fromInt(value)); });
        return spinBox;
    }

    if (field.kindType == SettingsFieldType::Float) {
        auto *doubleSpinBox = new QDoubleSpinBox();
        doubleSpinBox->setRange(0.0, 1000.0); // TODO: schemaにmin/maxが乗ったらそちらを使う
        doubleSpinBox->setValue(currentValue.asFloat());
        QObject::connect(doubleSpinBox, QOverload<double>::of(&QDoubleSpinBox::valueChanged), doubleSpinBox,
                         [binding](double value) { binding.setValue(FieldValue::fromFloat(value)); });
        return doubleSpinBox;
    }

    if (field.kindType == SettingsFieldType::Enum) {
        auto *comboBox = new QComboBox();
        const QStringList options = guessEnumOptions(field.key, currentValue.asQString());
        comboBox->addItems(options);
        comboBox->setCurrentText(currentValue.asQString());
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
            color = QColor::fromRgbF(rgba.r, rgba.g, rgba.b, rgba.a);

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

    // String, および未対応種別(FilePath/Array/Map)のフォールバック
    auto *lineEdit = new QLineEdit();
    lineEdit->setText(currentValue.asQString());
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
