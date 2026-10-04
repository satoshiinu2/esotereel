#pragma once

#include "ffi/FieldValue.h"
#include <QDialog>

class QTableWidget;
class QPushButton;

class LogFilterDialog : public QDialog {
    Q_OBJECT

  public:
    explicit LogFilterDialog(QWidget *parent = nullptr);
    void setFilters(const esotereel::FieldValue::Map &filters);
    esotereel::FieldValue::Map filters() const;

  private slots:
    void addFilter();
    void removeFilter(int row);

  private:
    QTableWidget *table;
    QPushButton *addButton;
};