#include "LogFilterDialog.h"

#include <QComboBox>
#include <QDialogButtonBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLineEdit>
#include <QPushButton>
#include <QTableWidget>
#include <QVBoxLayout>

#include "esotereel_gui_helper.h"

using LogLevel = esotereel_gui_helper::CLogLevel;

LogFilterDialog::LogFilterDialog(QWidget *parent) : QDialog(parent) {
    setWindowTitle("Log Filters");
    resize(500, 350);

    table = new QTableWidget(this);
    table->setColumnCount(3);
    table->setHorizontalHeaderLabels({"Target", "Level", ""});

    table->horizontalHeader()->setStretchLastSection(false);
    table->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Stretch);
    table->horizontalHeader()->setSectionResizeMode(1, QHeaderView::ResizeToContents);
    table->horizontalHeader()->setSectionResizeMode(2, QHeaderView::Fixed);
    table->setColumnWidth(2, 40);

    table->verticalHeader()->setVisible(false);
    table->setSelectionMode(QAbstractItemView::NoSelection);
    table->setEditTriggers(QAbstractItemView::DoubleClicked | QAbstractItemView::EditKeyPressed);

    addButton = new QPushButton("+", this);
    addButton->setFixedWidth(40);

    connect(addButton, &QPushButton::clicked, this, &LogFilterDialog::addFilter);

    auto *buttonLayout = new QHBoxLayout;
    buttonLayout->addWidget(addButton);
    buttonLayout->addStretch();

    auto *layout = new QVBoxLayout(this);
    layout->addWidget(table);
    layout->addLayout(buttonLayout);

    auto *dialogButtons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, this);
    connect(dialogButtons, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(dialogButtons, &QDialogButtonBox::rejected, this, &QDialog::reject);
    layout->addWidget(dialogButtons);

    setLayout(layout);
}

void LogFilterDialog::setFilters(const esotereel::FieldValue::Map &filters) {
    table->setRowCount(0);
    for (const auto &[targetName, filterValue] : filters) {
        addFilter();
        const int row = table->rowCount() - 1;
        auto *target = qobject_cast<QLineEdit *>(table->cellWidget(row, 0));
        auto *level = qobject_cast<QComboBox *>(table->cellWidget(row, 1));
        target->setText(QString::fromStdString(targetName));
        level->setCurrentText(QString::fromStdString(filterValue.asString()));
    }
}

esotereel::FieldValue::Map LogFilterDialog::filters() const {
    esotereel::FieldValue::Map filters;
    for (int row = 0; row < table->rowCount(); ++row) {
        const auto *target = qobject_cast<QLineEdit *>(table->cellWidget(row, 0));
        const auto *level = qobject_cast<QComboBox *>(table->cellWidget(row, 1));
        const QString targetName = target->text().trimmed();
        if (!targetName.isEmpty()) {
            filters[targetName.toStdString()] = esotereel::FieldValue::fromEnum(level->currentText().toStdString());
        }
    }
    return filters;
}

void LogFilterDialog::addFilter() {
    const int row = table->rowCount();

    table->insertRow(row);

    // Target
    auto *target = new QLineEdit(table);
    target->setPlaceholderText("target");

    table->setCellWidget(row, 0, target);

    // Level
    auto *level = new QComboBox(table);

    level->addItem("Off", static_cast<int>(LogLevel::Off));
    level->addItem("Error", static_cast<int>(LogLevel::Error));
    level->addItem("Warn", static_cast<int>(LogLevel::Warn));
    level->addItem("Info", static_cast<int>(LogLevel::Info));
    level->addItem("Debug", static_cast<int>(LogLevel::Debug));
    level->addItem("Trace", static_cast<int>(LogLevel::Trace));

    level->setCurrentText("Info");

    table->setCellWidget(row, 1, level);

    // Remove button
    auto *remove = new QPushButton("×", table);
    remove->setFixedWidth(40);

    table->setCellWidget(row, 2, remove);

    connect(remove, &QPushButton::clicked, this, [this, remove]() {
        for (int row = 0; row < table->rowCount(); ++row) {
            if (table->cellWidget(row, 2) == remove) {
                removeFilter(row);
                return;
            }
        }
    });
}

void LogFilterDialog::removeFilter(int row) {
    table->removeRow(row);
}