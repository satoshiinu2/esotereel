#include "../../../Utils.h"
#include "../../../wrapper/ClientNetworkHandler.h"
#include "TimelineWidget.h"

namespace esotereel::window {
QRect TimelineWidget::getInnerRect() const noexcept {
    QRect innerRect = rect();
    innerRect.setLeft(LABEL_WIDTH);
    innerRect.setTop(RULER_HEIGHT);
    return innerRect;
}

void TimelineWidget::drawLayers(const Project &project, QPainter &p, const QRect &r) const {
    QRect bgRect = getInnerRect();
    bgRect.setLeft(0);
    QRect innerRect = getInnerRect();

    std::unique_ptr<RenderRows> &rr = this->cachedRows;
    if (!rr) {
        return;
    }

    size_t rowIdx = 0;
    for (auto const &row : rr->rows()) {
        p.setClipRect(bgRect);
        double_t y = r.top() + this->rowToY(rowIdx);

        // コンテンツ背景
        QRect contentArea(r.left() + LABEL_WIDTH, y, r.width() - LABEL_WIDTH, LAYER_HEIGHT);
        p.fillRect(contentArea, rowContentBackgroundColor(rowIdx));

        // クリップ描画
        for (auto const &clip : rr->clipsFor(row)) {
            drawClip(clip, p, r, y);
        }

        // レイヤーラベル
        drawRowLabel(project, row, p, r, y);

        rowIdx++;
    }

    p.setPen(palette().mid().color());
    p.drawLine(r.left() + LABEL_WIDTH, r.top(), r.left() + LABEL_WIDTH, r.bottom());
    p.setClipping(false);
}

// 行の縞模様の背景色。paletteの基準色を、明暗どちらのテーマでも
// 視認しやすい方向にlighter/darkerで補正する。
QColor TimelineWidget::rowContentBackgroundColor(size_t rowIdx) const noexcept {
    QColor contentColor = (rowIdx % 2 == 0) ? palette().base().color() : palette().alternateBase().color();
    if (contentColor.lightness() < 128) {
        contentColor = contentColor.lighter(150);
    } else {
        contentColor = contentColor.darker(125);
    }
    return contentColor;
}

// レイヤーラベル領域(背景・インデント・フォルダーの開閉矢印・名前)を1行分描画する。
void TimelineWidget::drawRowLabel(const Project &project, const FfiLayerRow &row, QPainter &p, const QRect &r,
                                  double_t y) const {
    QRect labelRect(r.left(), y, LABEL_WIDTH, LAYER_HEIGHT);
    p.fillRect(labelRect, getLabelBgColor());

    // インデント + フォルダー▶▼
    int textX = r.left() + 4 + row.depth * INDENT_WIDTH;
    QString label;
    if (row.is_folder) {
        label = (row.is_folder_open ? QStringLiteral("\u25BC ") : QStringLiteral("\u25B6 "));
    }
    label += project.timelineOf(this->timelineIdx).layerById(row.layer_id).name();

    QRect textRect(textX, y, LABEL_WIDTH - textX, LAYER_HEIGHT);
    p.setPen(palette().text().color());
    p.drawText(textRect, Qt::AlignVCenter | Qt::AlignLeft, label);
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
        QRectF textRect(x + 2.0, r.top(), 100.0, RULER_HEIGHT);
        p.drawText(textRect, Qt::AlignTop | Qt::AlignLeft, text);
    }
}

void TimelineWidget::paintEvent(QPaintEvent *e) {
    QWidget::paintEvent(e);

    this->updateSnapshot();

    QPainter p(this);
    QRect r = rect();
    // 背景
    p.fillRect(r, palette().window());

    // ルーラー
    this->drawRuler(p, r);

    auto projectResult = this->windowState.network->getProject();
    if (projectResult.isError()) {
        // Lock is busy - skip project rendering and continue with basic UI elements
        // This prevents UI deadlock when network thread holds write lock
        // 選択エリア
        this->drawSelectionRect(p, r);

        // 再生ヘッド
        this->drawPlayhead(this->playhead, p, r);
        return;
    }
    auto project = projectResult.unwrapOrMove();
    if (project.isValid()) {
        // レイヤーラベルと区切り線
        this->drawLayers(project, p, r);

        // ゴースト
        this->drawDragGhost(project, p, r);
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
} // namespace esotereel::window