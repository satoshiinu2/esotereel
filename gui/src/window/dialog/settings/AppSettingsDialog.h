#pragma once

#include "ffi/AppSettingsFiles.h"
#include <QDialog>
#include <QTabWidget>
#include <QTableWidget>
#include <QVector>

class QPushButton;

namespace esotereel::window {
struct WindowGState;
}

namespace esotereel::window::dialog {

// アプリ設定（階層 TOML）のエディタです。User / Project の2タブで、
// 宣言済みキーを層ごとに編集します。値の編集コントロールは
// FieldControlFactory が生成し、スコープ外のキーはそのタブで
// 無効表示になり、既定値へのリセットもできます。
class AppSettingsDialog : public QDialog {
    Q_OBJECT

  public:
    explicit AppSettingsDialog(WindowGState &windowState, QWidget *parent = nullptr);
    ~AppSettingsDialog() override = default;

  private:
    void loadLayer(int layer);
    void commitFieldValue(int layer, const QString &key, const FieldValue &value);
    // コントロールを作り直さずに、実効値の表示とリセット可否だけを
    // 最新の状態へ合わせます（編集の最中に呼ばれても安全な範囲）。
    void refreshRowTexts(int layer);
    void resetKey(int layer, const QString &key);

    WindowGState &windowState;
    QTabWidget *tabs;
    QTableWidget *userTable;
    QTableWidget *projectTable;
    QVector<AppSettingEntry> entries;
    QVector<QPushButton *> resetButtons;
};

} // namespace esotereel::window::dialog
