#include "SettingsDialog.h"
#include "window/MainWindow.h"
#include <QCheckBox>
#include <QColor>
#include <QColorDialog>
#include <QComboBox>
#include <QDoubleSpinBox>
#include <QFont>
#include <QFormLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLabel>
#include <QLineEdit>
#include <QScrollArea>
#include <QSpinBox>
#include <QVBoxLayout>

namespace esotereel::window::dialog {

using SettingsFieldType = esotereel_gui_helper::SettingsFieldType;

SettingsDialog::SettingsDialog(WindowGState &windowState, QWidget *parent) : QDialog(parent), windowState(windowState) {
    setWindowTitle("Settings");
    resize(900, 600);

    setupUI();
    loadSettings();
}

void SettingsDialog::setupUI() {
    auto *mainLayout = new QVBoxLayout(this);

    // Search bar
    auto *searchLayout = new QHBoxLayout();
    searchLayout->addWidget(new QLabel("Search:"));
    searchEdit = new QLineEdit();
    searchEdit->setPlaceholderText("Search settings...");
    connect(searchEdit, &QLineEdit::textChanged, this, &SettingsDialog::onSearchTextChanged);
    searchLayout->addWidget(searchEdit);
    mainLayout->addLayout(searchLayout);

    // Splitter for categories and settings
    splitter = new QSplitter(Qt::Horizontal);

    // Category tree (left side)
    categoryTree = new QTreeWidget();
    categoryTree->setHeaderLabel("Categories");
    categoryTree->setMaximumWidth(250);
    categoryTree->setMinimumWidth(150);
    connect(categoryTree, &QTreeWidget::itemClicked, this, &SettingsDialog::onCategorySelected);
    splitter->addWidget(categoryTree);

    // Settings list (right side)
    settingsList = new QListWidget();
    settingsList->setAlternatingRowColors(true);
    splitter->addWidget(settingsList);

    splitter->setStretchFactor(0, 0);
    splitter->setStretchFactor(1, 1);
    mainLayout->addWidget(splitter);

    // Buttons
    auto *buttonLayout = new QHBoxLayout();
    buttonLayout->addStretch();

    resetButton = new QPushButton("Reset to Defaults");
    connect(resetButton, &QPushButton::clicked, this, &SettingsDialog::onResetButtonClicked);
    buttonLayout->addWidget(resetButton);

    okButton = new QPushButton("OK");
    connect(okButton, &QPushButton::clicked, this, &SettingsDialog::onOkButtonClicked);
    buttonLayout->addWidget(okButton);

    applyButton = new QPushButton("Apply");
    connect(applyButton, &QPushButton::clicked, this, &SettingsDialog::onApplyButtonClicked);
    buttonLayout->addWidget(applyButton);

    cancelButton = new QPushButton("Cancel");
    connect(cancelButton, &QPushButton::clicked, this, &QDialog::reject);
    buttonLayout->addWidget(cancelButton);

    mainLayout->addLayout(buttonLayout);
}

void SettingsDialog::loadSettings() {
    auto fieldsResult = Settings::getAllFields(windowState.network);
    if (fieldsResult.isOk()) {
        allFields = fieldsResult.unwrap();
        qDebug() << "Loaded" << allFields.size() << "settings fields";
        for (const auto &field : allFields) {
            qDebug() << "Field:" << field.key << "Category:" << field.category << "Label:" << field.label;
        }
    } else {
        qWarning() << "Failed to get fields:" << fieldsResult.errorMessage().value_or("Unknown error");
    }

    // Group fields by category
    fieldsByCategory.clear();
    for (const auto &field : allFields) {
        QString category = field.category.isEmpty() ? "General" : field.category;
        fieldsByCategory[category].append(field);
    }

    qDebug() << "Categories:" << fieldsByCategory.keys();
    populateCategories();
}

void SettingsDialog::populateCategories() {
    categoryTree->clear();

    QStringList categories;
    auto categoriesResult = Settings::getCategories(windowState.network);
    if (categoriesResult.isOk()) {
        categories = categoriesResult.unwrap();
        qDebug() << "Got categories from Settings:" << categories;
    } else {
        qWarning() << "Failed to get categories:" << categoriesResult.errorMessage().value_or("Unknown error");
    }

    if (categories.isEmpty()) {
        // Fallback to categories from fields
        categories = fieldsByCategory.keys();
        qDebug() << "Using fallback categories from fields:" << categories;
    }

    for (const QString &category : categories) {
        auto *item = new QTreeWidgetItem(categoryTree);
        item->setText(0, category);
        item->setData(0, Qt::UserRole, category);
    }

    qDebug() << "Total categories in tree:" << categoryTree->topLevelItemCount();

    // Select first category
    if (categoryTree->topLevelItemCount() > 0) {
        categoryTree->setCurrentItem(categoryTree->topLevelItem(0));
        populateSettings(categoryTree->topLevelItem(0)->text(0));
    }
}

void SettingsDialog::populateSettings(const QString &category) {
    settingsList->clear();

    QString searchCategory = category.isEmpty() ? "General" : category;
    QVector<SettingsField> fields = fieldsByCategory.value(searchCategory);

    qDebug() << "Populating settings for category:" << searchCategory << "with" << fields.size() << "fields";

    for (const auto &field : fields) {
        auto *item = new QListWidgetItem(settingsList);
        item->setText(field.label);
        item->setData(Qt::UserRole, QVariant::fromValue(field));

        // Create custom widget for this setting
        QWidget *widget = createControlForField(field);
        item->setSizeHint(widget->sizeHint());
        settingsList->setItemWidget(item, widget);
    }

    qDebug() << "Total items in settings list:" << settingsList->count();
}

QWidget *SettingsDialog::createControlForField(const SettingsField &field) {
    auto *container = new QWidget();
    auto *layout = new QVBoxLayout(container);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(2);

    auto *nameLabel = new QLabel(field.label);
    QFont labelFont = nameLabel->font();
    labelFont.setBold(true);
    nameLabel->setFont(labelFont);
    layout->addWidget(nameLabel);

    // Get current value
    QString currentValue = field.defaultValue;
    auto valueResult = Settings::getValue(windowState.network, field.key);
    if (valueResult.isOk()) {
        currentValue = valueResult.unwrap();
    }

    QWidget *control = nullptr;

    if (field.kindType == SettingsFieldType::Bool) {
        auto *checkBox = new QCheckBox();
        bool checked = (currentValue == "true");
        checkBox->setChecked(checked);
        checkBox->setProperty("settingKey", field.key);
        connect(checkBox, &QCheckBox::checkStateChanged, this, [this, checkBox](Qt::CheckState state) {
            QString key = checkBox->property("settingKey").toString();
            QString value = (state == Qt::Checked) ? "true" : "false";
            Settings::setValue(windowState.network, key, value);
        });
        control = checkBox;
    } else if (field.kindType == SettingsFieldType::Int) {
        auto *spinBox = new QSpinBox();
        spinBox->setRange(0, 1000); // Default range, should be parsed from schema
        spinBox->setValue(currentValue.toInt());
        spinBox->setProperty("settingKey", field.key);
        connect(spinBox, QOverload<int>::of(&QSpinBox::valueChanged), this, [this, spinBox](int value) {
            QString key = spinBox->property("settingKey").toString();
            Settings::setValue(windowState.network, key, QString::number(value));
        });
        control = spinBox;
    } else if (field.kindType == SettingsFieldType::Float) {
        auto *doubleSpinBox = new QDoubleSpinBox();
        doubleSpinBox->setRange(0.0, 1000.0); // Default range
        doubleSpinBox->setValue(currentValue.toDouble());
        doubleSpinBox->setProperty("settingKey", field.key);
        connect(doubleSpinBox, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this,
                [this, doubleSpinBox](double value) {
                    QString key = doubleSpinBox->property("settingKey").toString();
                    Settings::setValue(windowState.network, key, QString::number(value));
                });
        control = doubleSpinBox;
    } else if (field.kindType == SettingsFieldType::Enum) {
        auto *comboBox = new QComboBox();
        // For enum types, we need to parse the options from the schema
        // This is a simplified version - in a real implementation, we'd parse the schema properly
        QStringList options;
        // Hardcode some common enum options based on the key
        if (field.key.contains("log_level") || field.key.contains("level")) {
            options << "Off" << "Error" << "Warn" << "Info" << "Debug" << "Trace";
        } else if (field.key.contains("theme")) {
            options << "Light" << "Dark" << "System";
        } else {
            // Fallback: try to parse from default value or use current value
            options << currentValue;
        }

        comboBox->addItems(options);
        comboBox->setCurrentText(currentValue);
        comboBox->setProperty("settingKey", field.key);
        connect(comboBox, &QComboBox::currentTextChanged, this, [this, comboBox](const QString &text) {
            QString key = comboBox->property("settingKey").toString();
            Settings::setValue(windowState.network, key, text);
        });
        control = comboBox;
    } else if (field.kindType == SettingsFieldType::String) {
        auto *lineEdit = new QLineEdit();
        lineEdit->setText(currentValue);
        lineEdit->setProperty("settingKey", field.key);
        connect(lineEdit, &QLineEdit::textChanged, this, [this, lineEdit](const QString &text) {
            QString key = lineEdit->property("settingKey").toString();
            Settings::setValue(windowState.network, key, text);
        });
        control = lineEdit;
    } else if (field.kindType == SettingsFieldType::Color) {
        auto *colorButton = new QPushButton("Choose Color");
        colorButton->setProperty("settingKey", field.key);
        connect(colorButton, &QPushButton::clicked, this, [this, colorButton]() {
            QString key = colorButton->property("settingKey").toString();
            QString currentColor = "#FFFFFF";
            auto colorResult = Settings::getValue(windowState.network, key);
            if (colorResult.isOk()) {
                currentColor = colorResult.unwrap();
            }
            QColor color = QColor(currentColor);
            QColorDialog dialog(color, this);
            if (dialog.exec() == QDialog::Accepted) {
                QColor selectedColor = dialog.selectedColor();
                Settings::setValue(windowState.network, key, selectedColor.name());
            }
        });
        control = colorButton;
    } else {
        // Default to line edit for unknown types
        auto *lineEdit = new QLineEdit();
        lineEdit->setText(currentValue);
        lineEdit->setProperty("settingKey", field.key);
        connect(lineEdit, &QLineEdit::textChanged, this, [this, lineEdit](const QString &text) {
            QString key = lineEdit->property("settingKey").toString();
            Settings::setValue(windowState.network, key, text);
        });
        control = lineEdit;
    }

    if (control) {
        layout->addWidget(control);
    }

    return container;
}

void SettingsDialog::onCategorySelected(QTreeWidgetItem *item, int column) {
    Q_UNUSED(column);
    if (item) {
        QString category = item->data(0, Qt::UserRole).toString();
        populateSettings(category);
    }
}

void SettingsDialog::onSearchTextChanged(const QString &text) {
    if (text.isEmpty()) {
        // Show all settings for current category
        if (categoryTree->currentItem()) {
            populateSettings(categoryTree->currentItem()->text(0));
        }
        return;
    }

    // Filter settings based on search text
    settingsList->clear();

    for (const auto &field : allFields) {
        if (field.label.contains(text, Qt::CaseInsensitive) || field.key.contains(text, Qt::CaseInsensitive)) {
            auto *item = new QListWidgetItem(settingsList);
            item->setText(field.label);
            item->setData(Qt::UserRole, QVariant::fromValue(field));

            QWidget *widget = createControlForField(field);
            item->setSizeHint(widget->sizeHint());
            settingsList->setItemWidget(item, widget);
        }
    }
}

void SettingsDialog::onApplyButtonClicked() {
    // Apply changes without closing the dialog
    // Settings are already applied in real-time via the connect() calls
    // This button could be used to persist settings to disk
    qDebug() << "Settings applied";
}

void SettingsDialog::onOkButtonClicked() {
    // Apply changes and close the dialog
    accept();
}

void SettingsDialog::onResetButtonClicked() {
    // Reset all settings to defaults
    for (const auto &field : allFields) {
        Settings::setValue(windowState.network, field.key, field.defaultValue);
    }

    // Refresh the current view
    if (categoryTree->currentItem()) {
        populateSettings(categoryTree->currentItem()->text(0));
    }
}

} // namespace esotereel::window::dialog