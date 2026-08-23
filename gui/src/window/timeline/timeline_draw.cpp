#include "../../util.h"
#include "../../wrapper/network.h"
#include "../../wrapper/project/clip_render_info.h" // IWYU pragma: keep
#include "../../wrapper/project/project.h"          // IWYU pragma: keep
#include "../../wrapper/project/timeline.h"         // IWYU pragma: keep
#include "../../wrapper/project/timeline_layers.h"  // IWYU pragma: keep
#include "../../wrapper/result.h"
#include "../main.h"
#include "esotereel_gui_helper.h"
#include "timeline.h"
#include <QBrush>
#include <QColor>
#include <QLine>
#include <QPainter>
#include <QVariant>
#include <QWidget>
#include <cmath>
#include <cstdint>

QRect TimelineWidget::getInnerRect() const noexcept {
    QRect innerRect = rect();
    innerRect.setLeft(LABEL_WIDTH);
    innerRect.setTop(RULER_HEIGHT);
    return innerRect;
}

void TimelineWidget::drawLayers(const Project &project, const Timeline &timeline, QPainter &p, const QRect &r) const {
    QRect bgRect = getInnerRect();
    bgRect.setLeft(0);
    QRect innerRect = getInnerRect();
    const RenderRows &rr = getRows(project, timeline);

    size_t rowIdx = 0;
    for (auto const &row : rr.rows()) {
        p.setClipRect(bgRect);
        double_t y = r.top() + this->rowToY(rowIdx);

        // コンテンツ背景
        QRect contentArea(r.left() + LABEL_WIDTH, y, r.width() - LABEL_WIDTH, LAYER_HEIGHT);
        QColor contentColor = (rowIdx % 2 == 0) ? palette().base().color() : palette().alternateBase().color();
        if (contentColor.lightness() < 128) {
            contentColor = contentColor.lighter(150);
        } else {
            contentColor = contentColor.darker(125);
        }
        p.fillRect(contentArea, contentColor);

        // レイヤーラベル
        QRect labelRect(r.left(), y, LABEL_WIDTH, LAYER_HEIGHT);
        p.fillRect(labelRect, getLabelBgColor());

        // インデント + Composite▶▼
        int textX = r.left() + 4 + row.depth * INDENT_WIDTH;
        QString label;
        // if (row.is_composite) {
        //     label = (row.is_open ? "▼ " : "▶ ");
        // }
        label += QString("Layer %1").arg(row.layer_order);

        QRect textRect(textX, y, LABEL_WIDTH - textX, LAYER_HEIGHT);
        p.setPen(palette().text().color());
        p.drawText(textRect, Qt::AlignVCenter | Qt::AlignLeft, label);

        // クリップ描画
        for (auto const &clip : rr.clipsFor(row)) {
            drawClip(clip, p, r, y);
        }
        rowIdx++;
    }

    p.setPen(palette().mid().color());
    p.drawLine(r.left() + LABEL_WIDTH, r.top(), r.left() + LABEL_WIDTH, r.bottom());
    p.setClipping(false);
}

void TimelineWidget::drawClip(const ClipRenderInfo &info, QPainter &p, const QRect &r, double_t y) const {
    bool isSelected = contains(this->selectedClipIds, info.clip_id);
    bool isDragging = std::holds_alternative<DragClip>(this->dragState);

    QColor bgColor;
    if (isSelected) {
        bgColor = isDragging ? QColor(70, 130, 180, 50) : QColor(100, 150, 200);
    } else {
        bgColor = QColor(70, 130, 180);
    }

    QColor strokeColor;
    if (isSelected) {
        strokeColor = isDragging ? QColor(100, 160, 210, 50) : QColor(150, 200, 255);
    } else {
        strokeColor = QColor(100, 160, 210);
    }

    double_t x = r.left() + this->frameToX(info.abs_frame);
    double_t w = info.duration * this->zoom;
    QRectF clipRect(x, y + 2.0, w, LAYER_HEIGHT - 4.0);

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

    QColor rulerBg = this->getLabelBgColor();
    p.fillRect(rulerRect, rulerBg);

    // 目盛り（10フレームごと）
    double_t startFrame = (this->scroll.x() / this->zoom);
    double_t endFrame = startFrame + (r.width() / this->zoom) + RULER_STEP;

    for (int frame = startFrame; frame < endFrame; frame += RULER_STEP) {
        double_t x = r.left() + this->frameToX(frame);
        if (x < r.left() + LABEL_WIDTH) {
            continue;
        }
        QPen pen(palette().mid().color());
        pen.setWidth(1);
        p.setPen(pen);

        p.drawLine(x, r.top(), x, r.top() + RULER_HEIGHT);

        p.setFont(QFont("Arial", 10));
        p.setPen(palette().windowText().color());

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
    p.fillRect(r, palette().window());

    // ルーラー
    this->drawRuler(p, r);

    auto projectResult = windowState.network->getProject();
    if (projectResult.isError()) {
        // 選択エリア
        this->drawSelectionRect(p, r);

        // 再生ヘッド
        this->drawPlayhead(this->playhead, p, r);
        return;
    }
    auto project = projectResult.unwrapOrMove();
    Timeline timeline = project.isValid() ? project.timelineOf(this->timelineIdx) : Timeline(nullptr);
    if (timeline.isValid()) {
        // レイヤーラベルと区切り線
        this->drawLayers(project, timeline, p, r);

        // ゴースト
        this->drawDragGhost(timeline, p, r);
    }
    // 選択エリア
    this->drawSelectionRect(p, r);

    // 再生ヘッド
    this->drawPlayhead(this->playhead, p, r);
}

QColor TimelineWidget::getLabelBgColor() const noexcept {
    QColor color = palette().window().color();
    if (color.lightness() < 128) {
        color = color.lighter(115);
    } else {
        color = color.darker(115);
    }
    return color;
}