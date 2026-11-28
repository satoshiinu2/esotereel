#include "FieldControlFactory.h"

#include <QCheckBox>
#include <QColor>
#include <QColorDialog>
#include <QComboBox>
#include <QDialog>
#include <QDoubleSpinBox>
#include <QFileDialog>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QPushButton>
#include <QSpinBox>
#include <QVBoxLayout>
#include <algorithm>

namespace esotereel::window::widget {

using esotereel::FieldValue;
using SettingsFieldType = esotereel_gui_helper::SettingsFieldType;

namespace {

bool isNilAssetRef(const std::array<uint8_t, 16> &bytes) {
    return std::all_of(bytes.begin(), bytes.end(), [](uint8_t byte) { return byte == 0; });
}

QString assetAvailabilityText(uint8_t availability) {
    switch (availability) {
    case 1:
        return QObject::tr("Managed");
    case 2:
        return QObject::tr("Missing");
    default:
        return QObject::tr("External");
    }
}

QString assetDisplayText(const esotereel::AssetEntry &entry) {
    return QStringLiteral("%1 (%2)").arg(entry.name, assetAvailabilityText(entry.availability));
}

} // namespace

AssetRefEditorWidget::AssetRefEditorWidget(const FieldBinding &binding, QWidget *parent)
    : QWidget(parent), binding_(binding) {
    auto *layout = new QHBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    label_ = new QLabel(this);
    layout->addWidget(label_, 1);
    auto *selectButton = new QPushButton(tr("Select..."), this);
    layout->addWidget(selectButton);
    connect(selectButton, &QPushButton::clicked, this, &AssetRefEditorWidget::openPicker);
    refreshFromBinding();
}

void AssetRefEditorWidget::refreshFromBinding() {
    const auto ref = binding_.getValue().asAssetRef();
    if (!ref.has_value()) {
        qWarning() << "AssetRefEditorWidget: expected AssetRef value but got different type";
        label_->setText(tr("?"));
        return;
    }
    if (isNilAssetRef(*ref)) {
        label_->setText(tr("(No asset selected)"));
        return;
    }
    if (binding_.listAssets) {
        for (const auto &entry : binding_.listAssets()) {
            if (entry.id == *ref) {
                label_->setText(assetDisplayText(entry));
                return;
            }
        }
    }
    label_->setText(tr("Unknown asset"));
}

void AssetRefEditorWidget::openPicker() {
    QDialog dialog(this);
    dialog.setWindowTitle(tr("Select Asset"));
    auto *layout = new QVBoxLayout(&dialog);
    auto *list = new QListWidget(&dialog);
    layout->addWidget(list);

    // 参照は文字列化せず、UUID のバイト列のまま項目に持たせる。
    const std::function<void()> refreshList = [this, list]() {
        list->clear();
        if (!binding_.listAssets) {
            return;
        }
        for (const auto &entry : binding_.listAssets()) {
            auto *item = new QListWidgetItem(assetDisplayText(entry), list);
            item->setData(Qt::UserRole, QByteArray(reinterpret_cast<const char *>(entry.id.data()),
                                                   static_cast<int>(entry.id.size())));
        }
    };
    refreshList();
    if (const auto current = binding_.getValue().asAssetRef()) {

        for (int i = 0; i < list->count(); ++i) {
            const QByteArray bytes = list->item(i)->data(Qt::UserRole).toByteArray();
            if (bytes.size() == 16 && std::equal(bytes.begin(), bytes.end(), current->begin())) {
                list->setCurrentRow(i);
            }
        }
    }

    const std::function<void(QListWidgetItem *)> acceptItem = [this, &dialog](QListWidgetItem *item) {
        if (!item) {
            return;
        }
        const QByteArray bytes = item->data(Qt::UserRole).toByteArray();
        if (bytes.size() != 16) {
            return;
        }
        std::array<uint8_t, 16> ref{};
        std::copy(bytes.begin(), bytes.end(), ref.begin());
        binding_.setValue(FieldValue::fromAssetRef(ref));
        refreshFromBinding();
        dialog.accept();
    };

    auto *buttons = new QHBoxLayout();
    if (binding_.registerAssetFile) {
        auto *registerButton = new QPushButton(tr("Register from File..."), &dialog);
        buttons->addWidget(registerButton);
        connect(registerButton, &QPushButton::clicked, &dialog, [this, &dialog, list, refreshList]() {
            const QString path = QFileDialog::getOpenFileName(&dialog, tr("Register Asset File"));
            if (path.isEmpty()) {
                return;
            }
            const auto registered = binding_.registerAssetFile(path);
            refreshList();
            if (registered.has_value()) {
                for (int i = 0; i < list->count(); ++i) {
                    const QByteArray bytes = list->item(i)->data(Qt::UserRole).toByteArray();
                    if (bytes.size() == 16 && std::equal(bytes.begin(), bytes.end(), registered->begin())) {
                        list->setCurrentRow(i);
                    }
                }
            }
        });
    }
    auto *clearButton = new QPushButton(tr("Clear"), &dialog);
    buttons->addWidget(clearButton);
    buttons->addStretch();
    auto *cancelButton = new QPushButton(tr("Cancel"), &dialog);
    buttons->addWidget(cancelButton);
    auto *okButton = new QPushButton(tr("OK"), &dialog);
    buttons->addWidget(okButton);
    layout->addLayout(buttons);

    connect(clearButton, &QPushButton::clicked, &dialog, [this, &dialog]() {
        binding_.setValue(FieldValue::fromAssetRef(std::array<uint8_t, 16>{}));
        refreshFromBinding();
        dialog.accept();
    });
    connect(cancelButton, &QPushButton::clicked, &dialog, &QDialog::reject);
    connect(okButton, &QPushButton::clicked, &dialog, [list, acceptItem]() { acceptItem(list->currentItem()); });
    connect(list, &QListWidget::itemDoubleClicked, &dialog, acceptItem);

    dialog.exec();
}

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
        if (boolValue.has_value()) {
            checkBox->setChecked(boolValue.value());
        } else {
            qWarning() << "FieldControlFactory: expected Bool value for" << field.key << "but got different type";
        }

        QObject::connect(checkBox, &QCheckBox::checkStateChanged, checkBox, [binding](Qt::CheckState state) {
            binding.setValue(FieldValue::fromBool(state == Qt::Checked));
        });
        return checkBox;
    }

    if (field.kindType == SettingsFieldType::Int) {
        auto *spinBox = new QSpinBox();
        spinBox->setRange(0, 1000); // TODO: schemaにmin/maxが乗ったらそちらを使う

        auto intValue = currentValue.asInt();
        if (intValue.has_value()) {
            spinBox->setValue(intValue.value());
        } else {
            qWarning() << "FieldControlFactory: expected Int value for" << field.key << "but got different type";
        }

        QObject::connect(spinBox, QOverload<int>::of(&QSpinBox::valueChanged), spinBox,
                         [binding](int value) { binding.setValue(FieldValue::fromInt(value)); });
        return spinBox;
    }

    if (field.kindType == SettingsFieldType::Float) {
        auto *doubleSpinBox = new QDoubleSpinBox();
        doubleSpinBox->setRange(-1.0e9, 1.0e9); // TODO: use schema min/max when exposed by FFI

        auto floatValue = currentValue.asFloat();
        if (floatValue.has_value()) {
            doubleSpinBox->setValue(floatValue.value());
        } else {
            qWarning() << "FieldControlFactory: expected Float value for" << field.key << "but got different type";
        }

        QObject::connect(doubleSpinBox, QOverload<double>::of(&QDoubleSpinBox::valueChanged), doubleSpinBox,
                         [binding](double value) { binding.setValue(FieldValue::fromFloat(value)); });
        return doubleSpinBox;
    }

    if (field.kindType == SettingsFieldType::Enum) {
        auto *comboBox = new QComboBox();

        auto qstringValue = currentValue.asQString();
        if (qstringValue.has_value()) {
            const QStringList options = guessEnumOptions(field.key, qstringValue.value());
            comboBox->addItems(options);
            comboBox->setCurrentText(qstringValue.value());
        } else {
            qWarning() << "FieldControlFactory: expected Enum value for" << field.key << "but got different type";
        }

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
            if (rgba.has_value()) {
                color = QColor::fromRgbF(rgba->r, rgba->g, rgba->b, rgba->a);
            } else {
                qWarning() << "FieldControlFactory: expected Color value but got different type";
            }

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
                                 (*latest)[index] = isInteger ? FieldValue::fromInt(static_cast<std::int64_t>(number))
                                                              : FieldValue::fromFloat(number);
                                 binding.setValue(FieldValue::fromArray(*latest));
                             });
        }
        return container;
    }

    if (field.kindType == SettingsFieldType::AssetRef) {
        return new AssetRefEditorWidget(binding);
    }

    if (field.kindType == SettingsFieldType::Map) {
        // Map型は簡易表示のみ。編集UIは将来の実装に残す。
        auto *lineEdit = new QLineEdit();
        lineEdit->setEnabled(false);
        if (auto mapValue = currentValue.asMap()) {
            QStringList parts;
            for (const auto &[k, v] : *mapValue) {
                QString valStr;
                if (auto s = v.asQString())
                    valStr = *s;
                else if (auto n = v.asInt())
                    valStr = QString::number(*n);
                else if (auto f = v.asFloat())
                    valStr = QString::number(*f);
                else
                    valStr = QObject::tr("?");
                parts << QString::fromStdString(k) + ": " + valStr;
            }
            lineEdit->setText(parts.join(", "));
        } else if (auto qstringValue = currentValue.asQString()) {
            lineEdit->setText(qstringValue.value());
        }
        return lineEdit;
    }

    // String および FilePath のフォールバック（どちらもQStringとして扱える）
    auto *lineEdit = new QLineEdit();
    auto qstringValue = currentValue.asQString();
    if (qstringValue.has_value()) {
        lineEdit->setText(qstringValue.value());
    } else {
        qWarning() << "FieldControlFactory: expected String value for" << field.key << "but got different type";
    }
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
