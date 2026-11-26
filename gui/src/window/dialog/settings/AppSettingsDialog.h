#pragma once

#include <QDialog>
#include <QTabWidget>
#include <QTableWidget>

namespace esotereel::window {
struct WindowGState;
}

namespace esotereel::window::dialog {

// アプリ設定（階層 TOML）のエディタです。User / Project の2タブで、
// 宣言済みキーを層ごとに編集します。スコープ外のキーはそのタブで
// 無効表示になり、既定値へのリセットもできます。
class AppSettingsDialog : public QDialog {
    Q_OBJECT

  public:
    explicit AppSettingsDialog(WindowGState &windowState, QWidget *parent = nullptr);
    ~AppSettingsDialog() override = default;

  private:
    void loadLayer(int layer);
    void commitValue(int layer, const QString &key, uint8_t kind, const QString &text);
    void resetKey(int layer, const QString &key);

    WindowGState &windowState;
    QTabWidget *tabs;
    QTableWidget *userTable;
    QTableWidget *projectTable;
};

} // namespace esotereel::window::dialog
