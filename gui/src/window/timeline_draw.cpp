#include "../wrapper/clip.h"
#include "../wrapper/project.h"
#include "../wrapper/timeline.h"
#include "timeline.h"
#include <cmath>
#include <cstdint>
#include <qbrush.h>
#include <qcolor.h>
#include <qline.h>
#include <qpainter.h>
#include <qvariant.h>
#include <set>

template <typename T>
bool contains(const std::vector<T> &vec, const T &value) {
    return std::find(vec.begin(), vec.end(), value) != vec.end();
}
template <typename T>
bool contains(const std::set<T> &set, const T &value) {
    return set.find(value) != set.end();
}

void TimelineWidget::drawLayers(MTimeline &timeline, QPainter &p, QRect &r) {
    size_t layer_idx = 0;
    for (auto const &layer : timeline.layers()) {
        float_t y = r.top() + this->layer_to_y(layer_idx);
        QRect layer_rect(r.left(), y, r.width(), LAYER_HEIGHT);

        // 背景色
        auto d = this->drag_state;
        auto is_drop_target = d.has_value() ? d->current_layer_idx == layer_idx && d->src_layer_idx != layer_idx : false;

        QColor bgColor(45, 45, 45);
        if (is_drop_target) {
            bgColor = QColor(60, 80, 60); // ハイライト
        } else if (layer_idx % 2 == 0) {
            bgColor = QColor(40, 40, 40);
        }

        p.fillRect(layer_rect, bgColor);

        // レイヤーラベル
        QPoint pos(r.left() + 4, y + LAYER_HEIGHT / 2);

        p.setPen(Qt::white);

        QRect textRect(r.left() + 4, y, r.width(), LAYER_HEIGHT);
        p.drawText(textRect, Qt::AlignVCenter | Qt::AlignLeft, layer.name());

        // クリップ描画
        for (auto const &clip : layer.clips()) {
            this->drawClip(layer_idx, clip, p, r);
        }

        layer_idx++;
    }
}

void TimelineWidget::drawClip(size_t layer_idx, MClip clip, QPainter &p, QRect &r) {
    float_t y = r.top() + this->layer_to_y(layer_idx);
    bool is_selected = contains(this->selected_clips, clip.id());
    auto d = this->drag_state;
    bool is_dragging = d.has_value() ? d->src_layer_idx == layer_idx && d->clip_idx == clip.id() : false;

    // ドラッグ中は元の位置に半透明で残す
    QColor bgColor(70, 130, 180);
    if (is_dragging) {
        bgColor = QColor(70, 130, 180, 50);
    } else if (is_selected) {
        bgColor = QColor(100, 150, 200);
    }

    QColor strokeColor(100, 160, 210);
    if (is_dragging) {
        strokeColor = QColor(100, 160, 210, 50);
    } else if (is_selected) {
        strokeColor = QColor(150, 200, 255);
    }

    int redius = 3;
    auto x = r.left() + this->frame_to_x(clip.position());
    auto w = clip.duration() * this->zoom;
    QRect clip_rect(x, y + 2.0, w, LAYER_HEIGHT - 4.0);

    p.setBrush(bgColor);
    p.setPen(QPen(strokeColor, 1));
    p.drawRoundedRect(clip_rect, redius, redius);
}

void TimelineWidget::drawPlayhead(int64_t playhead_frame, QPainter &p, QRect &r) {
    float_t ph_x = r.left() + this->frame_to_x(playhead_frame);

    QPen pen(QColor(255, 80, 80));
    pen.setWidth(2);

    p.setPen(pen);

    p.drawLine(ph_x, r.top(), ph_x, r.bottom());
}

void TimelineWidget::drawRuler(QPainter &p, QRect &r) {
    QRect rulerRect(r.left() + LABEL_WIDTH, r.top(), r.width() - LABEL_WIDTH, RULER_HEIGHT);

    p.fillRect(rulerRect, QColor(50, 50, 50));

    // 目盛り（10フレームごと）
    const double_t SIZE = 10;
    double_t start_frame = (this->scroll.x() / this->zoom);
    double_t end_frame = start_frame + (r.width() / this->zoom) + SIZE;

    for (int frame = start_frame; frame < end_frame; frame += SIZE) {
        float_t x = r.left() + this->frame_to_x(frame);
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

        p.drawText(pos, text);
    }
}

void TimelineWidget::drawSelectionRect(QPainter &p, QRect &r) {
    if (!this->selection_rect.has_value()) {
        return;
    }
    auto sel = selection_rect.value();

    QRectF selRect(r.topLeft() + sel.start, r.topLeft() + sel.current);

    p.setBrush(QColor(100, 150, 255, 64));
    p.setPen(QPen(QColor(100, 150, 255), 1));
    p.drawRect(selRect);
}

void TimelineWidget::paintEvent(QPaintEvent *) {
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
        // this.drawGhost(timeline, p, r);
    }
    // 選択エリア
    this->drawSelectionRect(p, r);

    // 再生ヘッド
    this->drawPlayhead(0, p, r);

    // スクロールバー
    // this->drawScrollbar(timeline_size, None, p, r);
}