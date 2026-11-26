#pragma once

#include <QDialog>
#include <QListWidget>
#include <QPushButton>
#include <QTimer>

namespace esotereel::window {
struct WindowGState;
}

namespace esotereel::window::dialog {

// オートセーブ世代の一覧から復元するダイアログです。
// 起動時に異常終了を検知した場合と、メニューから任意に開く場合の
// 両方で使います。
class RecoveryDialog : public QDialog {
    Q_OBJECT

  public:
    explicit RecoveryDialog(WindowGState &windowState, QWidget *parent = nullptr);
    ~RecoveryDialog() override = default;

  private slots:
    void pollGenerations();
    void onRestoreClicked();
    void onNewProjectClicked();

  private:
    WindowGState &windowState;
    QListWidget *generationList;
    QPushButton *restoreButton;
    QTimer *pollTimer;
    int pollAttempts = 0;
};

} // namespace esotereel::window::dialog
