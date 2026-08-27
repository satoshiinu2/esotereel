#pragma once

#include <QDialog>

class QTableWidget;
class QPushButton;

class LogFilterDialog : public QDialog {
    Q_OBJECT

  public:
    explicit LogFilterDialog(QWidget *parent = nullptr);

  private slots:
    void addFilter();
    void removeFilter(int row);

  private:
    QTableWidget *table;
    QPushButton *addButton;
};