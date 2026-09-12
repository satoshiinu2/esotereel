#pragma once

#include <QDialog>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QPushButton>
#include <QSplitter>
#include <QTreeWidget>
#include <QVBoxLayout>

#include "ffi/Settings.h"

namespace esotereel {
class ClientState;
}

namespace esotereel::window {
struct WindowGState;
}

namespace esotereel::window::dialog {

class SettingsDialog : public QDialog {
    Q_OBJECT

  public:
    explicit SettingsDialog(WindowGState &windowState, QWidget *parent = nullptr);
    ~SettingsDialog() override = default;

  private slots:
    void onCategorySelected(QTreeWidgetItem *item, int column);
    void onSearchTextChanged(const QString &text);
    void onApplyButtonClicked();
    void onOkButtonClicked();
    void onResetButtonClicked();

  private:
    void setupUI();
    void loadSettings();
    void populateCategories();
    void populateSettings(const QString &category = QString());
    void createSettingControl(const QListWidgetItem *item, const esotereel::SettingsField &field);
    QWidget *createControlForField(const esotereel::SettingsField &field);

    WindowGState &windowState;
    QSplitter *splitter;
    QTreeWidget *categoryTree;
    QListWidget *settingsList;
    QLineEdit *searchEdit;
    QPushButton *applyButton;
    QPushButton *resetButton;
    QPushButton *okButton;
    QPushButton *cancelButton;

    QVector<esotereel::SettingsField> allFields;
    QMap<QString, QVector<esotereel::SettingsField>> fieldsByCategory;
};

} // namespace esotereel::window::dialog
