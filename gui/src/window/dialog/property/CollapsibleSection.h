#pragma once

#include <QToolButton>
#include <QVBoxLayout>
#include <QWidget>

namespace esotereel::window::widget {

// 画像の「位置とサイズ ⌄」「ブレンド ⌄」のような、見出しクリックで開閉するセクション。
// contentLayout() に中身(フィールドの行など)を積んでいく。
class CollapsibleSection : public QWidget {
    Q_OBJECT

  public:
    explicit CollapsibleSection(const QString &title, QWidget *parent = nullptr);

    QVBoxLayout *contentLayout() { return contentLayout_; }

  private:
    QToolButton *headerButton;
    QWidget *contentWidget;
    QVBoxLayout *contentLayout_;
};

} // namespace esotereel::window::widget
