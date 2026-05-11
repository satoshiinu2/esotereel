#pragma once

#include <QScrollBar>
#include <QWidget>

class DebugStreamsWidget : public QWidget {
    Q_OBJECT

  public:
    explicit DebugStreamsWidget(QWidget *parent = nullptr);

  protected:
    void paintEvent(QPaintEvent *e) override;

  private:
    QScrollBar *hScrollBar;
};