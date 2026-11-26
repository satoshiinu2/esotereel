#include "AppSettingsDialog.h"
#include "ffi/AppSettingsFiles.h"
#include "window/MainWindow.h"

#include <QCheckBox>
#include <QHeaderView>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QPushButton>
#include <QVBoxLayout>

namespace esotereel::window::dialog {

namespace {
constexpr int kLayerUser = 0;
constexpr int kLayerProject = 1;
constexpr uint8_t kKindBool = 2;

QString scopeText(uint8_t scope) {
    switch (scope) {
    case 0:
        return QStringLiteral("user only");
    case 1:
        return QStringLiteral("project/server only");
    default:
        return QStringLiteral("overridable");
    }
}
} // namespace

AppSettingsDialog::AppSettingsDialog(WindowGState &windowState, QWidget *parent)
    : QDialog(parent), windowState(windowState) {
    setWindowTitle(tr("App Settings"));
    resize(760, 420);

    auto *mainLayout = new QVBoxLayout(this);
    mainLayout->addWidget(new QLabel(
        tr("Layered app settings (defaults < user < project). Out-of-scope keys are "
           "disabled in that tab. Clearing a value resets the key to its default.")));

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

    auto result = AppSettingsFiles::list(windowState.state, static_cast<uint8_t>(layer));
    if (result.isError()) {
        table->setRowCount(1);
        auto *item = new QTableWidgetItem(
            tr("Unavailable: %1").arg(QString::fromStdString(*result.getError())));
        table->setItem(0, 0, item);
        table->setSpan(0, 0, 1, 5);
        return;
    }

    const auto entries = result.unwrap();
    table->setRowCount(entries.size());
    for (int row = 0; row < entries.size(); ++row) {
        const auto &entry = entries[row];
        auto *keyItem = new QTableWidgetItem(entry.key);
        keyItem->setFlags(Qt::ItemIsEnabled);
        if (!entry.isSet) {
            keyItem->setToolTip(tr("Not set in this layer"));
        }
        table->setItem(row, 0, keyItem);

        // 値の編集コントロール。Bool はチェックボックス、それ以外は1行入力。
        if (entry.kind == kKindBool) {
            auto *check = new QCheckBox();
            check->setChecked(entry.isSet ? entry.layerValue == QLatin1String("true")
                                          : entry.effectiveValue == QLatin1String("true"));
            check->setEnabled(entry.editableHere);
            connect(check, &QCheckBox::toggled, this, [this, layer, key = entry.key](bool on) {
                commitValue(layer, key, kKindBool, on ? QStringLiteral("true")
                                                      : QStringLiteral("false"));
            });
            table->setCellWidget(row, 1, check);
        } else {
            auto *edit = new QLineEdit(entry.layerValue);
            edit->setPlaceholderText(entry.effectiveValue);
            edit->setEnabled(entry.editableHere);
            connect(edit, &QLineEdit::editingFinished, this,
                    [this, layer, key = entry.key, kind = entry.kind, edit]() {
                        commitValue(layer, key, kind, edit->text().trimmed());
                    });
            table->setCellWidget(row, 1, edit);
        }

        auto *effectiveItem = new QTableWidgetItem(entry.effectiveValue);
        effectiveItem->setFlags(Qt::ItemIsEnabled);
        table->setItem(row, 2, effectiveItem);
        auto *defaultItem = new QTableWidgetItem(entry.defaultValue);
        defaultItem->setFlags(Qt::ItemIsEnabled);
        table->setItem(row, 3, defaultItem);

        auto *resetButton = new QPushButton(tr("Reset"));
        resetButton->setEnabled(entry.editableHere && entry.isSet);
        resetButton->setToolTip(scopeText(entry.scope));
        connect(resetButton, &QPushButton::clicked, this,
                [this, layer, key = entry.key]() { resetKey(layer, key); });
        table->setCellWidget(row, 4, resetButton);
    }
}

void AppSettingsDialog::commitValue(int layer, const QString &key, uint8_t kind,
                                    const QString &text) {
    Result<void> result = Result<void>::ok();
    if (text.isEmpty() && kind != kKindBool) {
        // 空にしたらその層の値を消す（既定値へ戻る）。
        result = AppSettingsFiles::reset(windowState.state, static_cast<uint8_t>(layer), key);
    } else {
        result = AppSettingsFiles::setValue(windowState.state, static_cast<uint8_t>(layer), key,
                                            text);
    }
    if (result.isError()) {
        QMessageBox::warning(this, tr("App Settings"),
                             QString::fromStdString(*result.getError()));
    }
    loadLayer(layer);
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
