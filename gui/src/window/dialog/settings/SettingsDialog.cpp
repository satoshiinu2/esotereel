#include "SettingsDialog.h"
#include "window/MainWindow.h"
#include "window/dialog/property/FieldControlFactory.h"

#include <QFont>
#include <QFormLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLabel>
#include <QScrollArea>
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
    auto fieldsResult = Settings::getAllFields(windowState.state);
    if (fieldsResult.isOk()) {
        allFields = fieldsResult.unwrap();
    } else {
        qWarning() << "Failed to get fields:" << QString::fromStdString(fieldsResult.error());
    }

    fieldsByCategory.clear();
    for (const auto &field : allFields) {
        QString category = field.category.isEmpty() ? "General" : field.category;
        fieldsByCategory[category].append(field);
    }

    populateCategories();
}

void SettingsDialog::populateCategories() {
    categoryTree->clear();

    QStringList categories;
    auto categoriesResult = Settings::getCategories(windowState.state);
    if (categoriesResult.isOk()) {
        categories = categoriesResult.unwrap();
    } else {
        qWarning() << "Failed to get categories:" << QString::fromStdString(categoriesResult.error());
    }

    if (categories.isEmpty()) {
        categories = fieldsByCategory.keys();
    }

    for (const QString &category : categories) {
        auto *item = new QTreeWidgetItem(categoryTree);
        item->setText(0, category);
        item->setData(0, Qt::UserRole, category);
    }

    if (categoryTree->topLevelItemCount() > 0) {
        categoryTree->setCurrentItem(categoryTree->topLevelItem(0));
        populateSettings(categoryTree->topLevelItem(0)->text(0));
    }
}

void SettingsDialog::populateSettings(const QString &category) {
    settingsList->clear();

    QString searchCategory = category.isEmpty() ? "General" : category;
    QVector<SettingsField> fields = fieldsByCategory.value(searchCategory);

    for (const auto &field : fields) {
        auto *item = new QListWidgetItem(settingsList);
        item->setText(field.label);
        item->setData(Qt::UserRole, QVariant::fromValue(field));

        QWidget *widget = createControlForField(field);
        item->setSizeHint(widget->sizeHint());
        settingsList->setItemWidget(item, widget);
    }
}

// フィールド1つ分の「ラベル + コントロール」の組み立てだけをここで行い、
// コントロール自体の生成とSettingsへのバインドはFieldControlFactoryに委譲する。
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

    widget::FieldBinding binding;
    binding.getValue = [this, field]() {
        FieldValue value = field.defaultValue;
        auto result = Settings::getValue(windowState.state, field.key);
        if (result.isOk()) {
            value = result.unwrap();
        }
        return value;
    };
    binding.setValue = [this, field](const FieldValue &value) {
        Settings::setValue(windowState.state, field.key, value);
    };

    QWidget *control = widget::FieldControlFactory::createControl(field, binding);
    layout->addWidget(control);

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
        if (categoryTree->currentItem()) {
            populateSettings(categoryTree->currentItem()->text(0));
        }
        return;
    }

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
    // Settings are already applied in real-time via the bindings above.
}

void SettingsDialog::onOkButtonClicked() {
    accept();
}

void SettingsDialog::onResetButtonClicked() {
    for (const auto &field : allFields) {
        Settings::setValue(windowState.state, field.key, field.defaultValue);
    }

    if (categoryTree->currentItem()) {
        populateSettings(categoryTree->currentItem()->text(0));
    }
}

} // namespace esotereel::window::dialog
