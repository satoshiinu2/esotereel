#include "LogFilterDialog.h"

#include <QComboBox>
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

    setLayout(layout);
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

    level->addItem("OFF", static_cast<int>(LogLevel::Off));
    level->addItem("ERROR", static_cast<int>(LogLevel::Error));
    level->addItem("WARN", static_cast<int>(LogLevel::Warn));
    level->addItem("INFO", static_cast<int>(LogLevel::Info));
    level->addItem("DEBUG", static_cast<int>(LogLevel::Debug));
    level->addItem("TRACE", static_cast<int>(LogLevel::Trace));

    level->setCurrentText("INFO");

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