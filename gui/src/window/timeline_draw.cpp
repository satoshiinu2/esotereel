#include "../util.h"
#include "timeline.h"
#include <cmath>
#include <cstdint>
#include <qbrush.h>
#include <qcolor.h>
#include <qline.h>
#include <qpainter.h>
#include <qvariant.h>
#include <qwidget.h>

QRect TimelineWidget::getInnerRect() const noexcept {
    QRect innerRect = rect();
    innerRect.setLeft(LABEL_WIDTH);
    innerRect.setTop(RULER_HEIGHT);
    return innerRect;
}

void TimelineWidget::drawLayers(const MTimeline &timeline, QPainter &p, const QRect &r) const {
    // setup clipping
    QRect bgRect = getInnerRect();
    bgRect.setLeft(0); // 左端まで
    QRect innerRect = getInnerRect();

    // draw main
    size_t layerIdx = 0;
    for (auto const &layer : timeline.layers()) {
        p.setClipRect(bgRect);
        double_t y = r.top() + this->layerToY(layerIdx);
        QRect layerRect(r.left(), y, r.width(), LAYER_HEIGHT);

        // 背景色
        QColor bgColor(45, 45, 45);
        if (layerIdx % 2 == 0) {
            bgColor = QColor(40, 40, 40);
        }

        p.fillRect(layerRect, bgColor);

        // レイヤーラベル
        QPoint pos(r.left() + 4, y + LAYER_HEIGHT / 2);

        p.setPen(Qt::white);

        QRect textRect(r.left() + 4, y, r.width(), LAYER_HEIGHT);
        p.drawText(textRect, Qt::AlignVCenter | Qt::AlignLeft, layer.name());

        // クリップ描画
        p.setClipRect(innerRect);
        for (auto const &clip : layer.clips()) {
            this->drawClip(layerIdx, clip, p, r);
        }

        layerIdx++;
    }
    p.setClipping(false);
}

void TimelineWidget::drawClip(size_t layer_idx, const MClip &clip, QPainter &p, const QRect &r) const {
    bool isSelected = contains(this->selectedClipIds, clip.id());
    bool isDragging = this->dragState.has_value();

    // ドラッグ中は元の位置に半透明で残す
    QColor bgColor;
    if (isSelected) {
        if (isDragging) {
            bgColor = QColor(70, 130, 180, 50);
        } else {
            bgColor = QColor(100, 150, 200);
        }
    } else {
        bgColor = QColor(70, 130, 180);
    }

    QColor strokeColor;
    if (isSelected) {
        if (isDragging) {
            strokeColor = QColor(100, 160, 210, 50);
        } else {
            strokeColor = QColor(150, 200, 255);
        }
    } else {
        strokeColor = QColor(100, 160, 210);
    }

    double_t x = r.left() + this->frameToX(clip.position());
    double_t y = r.top() + this->layerToY(layer_idx);
    double_t w = clip.duration() * this->zoom;
    QRect clipRect(x, y + 2.0, w, LAYER_HEIGHT - 4.0);

    p.setBrush(bgColor);
    p.setPen(QPen(strokeColor, 1));
    p.drawRoundedRect(clipRect, CLIP_ROUND_RADIUS, CLIP_ROUND_RADIUS);
}

void TimelineWidget::drawPlayhead(int64_t playhead_frame, QPainter &p, const QRect &r) const {
    QRect innerRect = getInnerRect();
    double_t drawPosX = r.left() + this->frameToX(playhead_frame);

    QPen pen(QColor(255, 80, 80));
    pen.setWidth(2);

    p.setPen(pen);

    p.setClipRect(innerRect);
    p.drawLine(drawPosX, r.top(), drawPosX, r.bottom());
    p.setClipping(false);
}

void TimelineWidget::drawRuler(QPainter &p, const QRect &r) const {
    QRect rulerRect(r.left() + LABEL_WIDTH, r.top(), r.width() - LABEL_WIDTH, RULER_HEIGHT);

    p.fillRect(rulerRect, QColor(50, 50, 50));

    // 目盛り（10フレームごと）
    double_t startFrame = (this->scroll.x() / this->zoom);
    double_t endFrame = startFrame + (r.width() / this->zoom) + RULER_STEP;

    for (int frame = startFrame; frame < endFrame; frame += RULER_STEP) {
        double_t x = r.left() + this->frameToX(frame);
        if (x < r.left() + LABEL_WIDTH) {
            continue;
        }
        QPen pen(QColor(100, 100, 100));
        pen.setWidth(1);
        p.setPen(pen);

        p.drawLine(x, r.top(), x, r.top() + RULER_HEIGHT);

        p.setFont(QFont("Arial", 10));
        p.setPen(QColor(180, 180, 180));

        QString text = QString::number(frame);
        QPointF pos(x + 2.0, r.top() + 4.0);

        QRectF textRect(x + 2.0, r.top(), 100.0, RULER_HEIGHT);
        p.drawText(textRect, Qt::AlignTop | Qt::AlignLeft, text);
    }
}

void TimelineWidget::paintEvent(QPaintEvent *e) {
    QWidget::paintEvent(e);

    QPainter p(this);
    QRect r = rect();
    // 背景
    p.fillRect(r, QColor(30, 30, 30));

    // ルーラー
    this->drawRuler(p, r);

    MProject project = getProject();
    MTimeline timeline = project.isValid() ? project.timelineOf(this->timelineType) : MTimeline(nullptr);
    if (timeline.isValid()) {
        // レイヤーラベルと区切り線
        this->drawLayers(timeline, p, r);

        // ゴースト
        this->drawDragGhost(timeline, p, r);
    }
    // 選択エリア
    this->drawSelectionRect(p, r);

    // 再生ヘッド
    this->drawPlayhead(this->playhead, p, r);
}