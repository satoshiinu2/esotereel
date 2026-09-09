#pragma once

#include <QHBoxLayout>
#include <QWidget>

namespace esotereel::window {

// タイムラインの外側・上部に置くツールバーの「置き場所」。
// 今はボタンの中身を持たない空のバー。後から
//   toolbar->buttonLayout()->insertWidget(toolbar->buttonInsertIndex(), new QToolButton(...));
// のように足していく想定。
class TimelineToolbarWidget : public QWidget {
    Q_OBJECT

  public:
    explicit TimelineToolbarWidget(QWidget *parent = nullptr) : QWidget(parent) {
        layout = new QHBoxLayout(this);
        layout->setContentsMargins(4, 2, 4, 2);
        layout->setSpacing(2);
        layout->addStretch(1); // ボタンは左詰め、右側は余白
    }

    // ボタンはこのレイアウトに addWidget/insertWidget していく。
    QHBoxLayout *buttonLayout() const {
        return layout;
    }
    // stretch(右側の余白)より前に挿入したい場合はこのindexを使う。
    int buttonInsertIndex() const {
        return layout->count() - 1;
    }

  private:
    QHBoxLayout *layout;
};

} // namespace esotereel::window
