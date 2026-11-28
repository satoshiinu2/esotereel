#include "AppSettingsDialog.h"
#include "ffi/AppSettingsFiles.h"
#include "ffi/Settings.h"
#include "window/MainWindow.h"
#include "window/dialog/property/FieldControlFactory.h"

#include <QHeaderView>
#include <QLabel>
#include <QMessageBox>
#include <QPushButton>
#include <QVBoxLayout>

namespace esotereel::window::dialog {

namespace {
constexpr int kLayerUser = 0;
constexpr int kLayerProject = 1;

QString scopeText(CSettingScope scope) {
    switch (scope) {
    case CSettingScope::UserOnly:
        return QStringLiteral("user only");
    case CSettingScope::ProjectOrServerOnly:
        return QStringLiteral("project/server only");
    case CSettingScope::Overridable:
        return QStringLiteral("overridable");
    }
    return QStringLiteral("overridable");
}

QString valueText(const std::optional<FieldValue> &value) {
    if (!value.has_value()) {
        return QString();
    }
    if (const auto b = value->asBool()) {
        return *b ? QStringLiteral("true") : QStringLiteral("false");
    }
    if (const auto i = value->asInt()) {
        return QString::number(*i);
    }
    if (const auto f = value->asFloat()) {
        return QString::number(*f);
    }
    if (const auto s = value->asQString()) {
        return *s;
    }
    return QStringLiteral("?");
}
} // namespace

AppSettingsDialog::AppSettingsDialog(WindowGState &windowState, QWidget *parent)
    : QDialog(parent), windowState(windowState) {
    setWindowTitle(tr("App Settings"));
    resize(760, 420);

    auto *mainLayout = new QVBoxLayout(this);
    mainLayout->addWidget(new QLabel(
        tr("Layered app settings (defaults < user < project). Out-of-scope keys are "
           "disabled in that tab. Resetting a value clears the key in that layer.")));

    tabs = new QTabWidget();
    userTable = new QTableWidget();
    projectTable = new QTableWidget();
    tabs->addTab(userTable, tr("User"));
    tabs->addTab(projectTable, tr("Project"));
    mainLayout->addWidget(tabs);

    auto *closeButton = new QPushButton(tr("Close"));
    connect(closeButton, &QPushButton::clicked, this, &QDialog::accept);
    mainLayout->addWidget(closeButton);

    connect(tabs, &QTabWidget::currentChanged, this,
            [this](int index) { loadLayer(index == 0 ? kLayerUser : kLayerProject); });
    loadLayer(kLayerUser);
}

void AppSettingsDialog::loadLayer(int layer) {
    QTableWidget *table = layer == kLayerUser ? userTable : projectTable;
    table->clear();
    table->setColumnCount(5);
    table->setHorizontalHeaderLabels(
        {tr("Key"), tr("Value"), tr("Effective"), tr("Default"), tr("Scope / Reset")});
    table->horizontalHeader()->setSectionResizeMode(QHeaderView::ResizeToContents);
    table->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Stretch);
    entries.clear();
    resetButtons.clear();

    auto result = AppSettingsFiles::list(windowState.state, static_cast<uint8_t>(layer));
    if (result.isError()) {
        table->setRowCount(1);
        auto *item = new QTableWidgetItem(
            tr("Unavailable: %1").arg(QString::fromStdString(*result.getError())));
        table->setItem(0, 0, item);
        table->setSpan(0, 0, 1, 5);
        return;
    }

    entries = result.unwrap();
    table->setRowCount(entries.size());
    for (int row = 0; row < entries.size(); ++row) {
        const auto &entry = entries[row];
        auto *keyItem = new QTableWidgetItem(entry.key);
        keyItem->setFlags(Qt::ItemIsEnabled);
        if (!entry.isSet) {
            keyItem->setToolTip(tr("Not set in this layer"));
        }
        table->setItem(row, 0, keyItem);

        // 値の編集コントロールはクリッププロパティと共通の Factory で作る。
        // 表示する値は、この層の値 → 実効値 → 既定値の順で最初にあるもの。
        SettingsField field;
        field.key = entry.key;
        field.label = entry.key;
        field.kindType = entry.kindType;
        if (entry.defaultValue.has_value()) {
            field.defaultValue = *entry.defaultValue;
        }
        widget::FieldBinding binding;
        binding.getValue = [this, row]() {
            const AppSettingEntry &current = entries[row];
            if (current.layerValue.has_value()) {
                return *current.layerValue;
            }
            if (current.effectiveValue.has_value()) {
                return *current.effectiveValue;
            }
            if (current.defaultValue.has_value()) {
                return *current.defaultValue;
            }
            return FieldValue::fromInt(0);
        };
        binding.setValue = [this, layer, key = entry.key](const FieldValue &value) {
            commitFieldValue(layer, key, value);
        };
        QWidget *control = widget::FieldControlFactory::createControl(field, binding);
        control->setEnabled(entry.editableHere);
        table->setCellWidget(row, 1, control);

        auto *effectiveItem = new QTableWidgetItem(valueText(entry.effectiveValue));
        effectiveItem->setFlags(Qt::ItemIsEnabled);
        table->setItem(row, 2, effectiveItem);
        auto *defaultItem = new QTableWidgetItem(valueText(entry.defaultValue));
        defaultItem->setFlags(Qt::ItemIsEnabled);
        table->setItem(row, 3, defaultItem);

        auto *resetButton = new QPushButton(tr("Reset"));
        resetButton->setEnabled(entry.editableHere && entry.isSet);
        resetButton->setToolTip(scopeText(entry.scope));
        connect(resetButton, &QPushButton::clicked, this,
                [this, layer, key = entry.key]() { resetKey(layer, key); });
        table->setCellWidget(row, 4, resetButton);
        resetButtons.push_back(resetButton);
    }
}

void AppSettingsDialog::commitFieldValue(int layer, const QString &key, const FieldValue &value) {
    auto result =
        AppSettingsFiles::setValue(windowState.state, static_cast<uint8_t>(layer), key, value);
    if (result.isError()) {
        QMessageBox::warning(this, tr("App Settings"),
                             QString::fromStdString(*result.getError()));
        loadLayer(layer);
        return;
    }
    refreshRowTexts(layer);
}

void AppSettingsDialog::refreshRowTexts(int layer) {
    auto result = AppSettingsFiles::list(windowState.state, static_cast<uint8_t>(layer));
    if (result.isError()) {
        return;
    }
    const auto fresh = result.unwrap();
    if (fresh.size() != entries.size()) {
        return;
    }
    entries = fresh;
    QTableWidget *table = layer == kLayerUser ? userTable : projectTable;
    for (int row = 0; row < entries.size(); ++row) {
        const auto &entry = entries[row];
        if (auto *keyItem = table->item(row, 0)) {
            keyItem->setToolTip(entry.isSet ? QString() : tr("Not set in this layer"));
        }
        if (auto *effectiveItem = table->item(row, 2)) {
            effectiveItem->setText(valueText(entry.effectiveValue));
        }
        if (row < resetButtons.size()) {
            resetButtons[row]->setEnabled(entry.editableHere && entry.isSet);
        }
    }
}

void AppSettingsDialog::resetKey(int layer, const QString &key) {
    auto result = AppSettingsFiles::reset(windowState.state, static_cast<uint8_t>(layer), key);
    if (result.isError()) {
        QMessageBox::warning(this, tr("App Settings"),
                             QString::fromStdString(*result.getError()));
    }
    loadLayer(layer);
}

} // namespace esotereel::window::dialog
