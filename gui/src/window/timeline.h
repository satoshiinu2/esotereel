#pragma once

#include "../wrapper/clip.h"
#include "../wrapper/timeline.h"
#include <QEvent>
#include <QPainter>
#include <QScrollBar>
#include <QWidget>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <set>

constexpr float_t LAYER_HEIGHT = 32.0;
constexpr float_t RULER_HEIGHT = 24.0;
constexpr float_t LABEL_WIDTH = 80.0;
constexpr float_t SCROLLBAR_SIZE = 12.0;

constexpr double_t DEFAULT_FRAME_COUNT = 300;
constexpr double_t DEFAULT_LAYER_LEN = 1;

struct SelectionRect {
    QPointF start;
    QPointF current;
};

class ClipDragState {
  public:
    size_t src_layer_idx;
    int64_t src_frame;
    size_t clip_idx;
    int64_t offset_frames;
    size_t current_layer_idx;
    int64_t current_frame;
    QPointF ghost_pos;
};

class TimelineWidget : public QWidget {
    Q_OBJECT

  public:
    size_t timelineType;
    float_t zoom;
    QPointF scroll;
    std::set<uint32_t> selected_clips; // clipid

    TimelineWidget(size_t timelineType);

    float_t frame_to_x(double_t frame) {
        return frame * this->zoom - this->scroll.x() + LABEL_WIDTH;
    }

    double_t x_to_frame(float_t x) {
        return ((x - LABEL_WIDTH + this->scroll.x()) / this->zoom);
    }

    float_t layer_to_y(size_t layer_idx) {
        return layer_idx * LAYER_HEIGHT + RULER_HEIGHT - this->scroll.y();
    }

  protected:
    void paintEvent(QPaintEvent *) override;
    void resizeEvent(QResizeEvent *) override;
    void mousePressEvent(QMouseEvent *e) override;
    void mouseReleaseEvent(QMouseEvent *e) override;
    void mouseMoveEvent(QMouseEvent *e) override;
    void wheelEvent(QWheelEvent *e) override;
    void mouseDoubleClickEvent(QMouseEvent *e) override;

  private:
    QScrollBar *hScrollBar;
    QScrollBar *vScrollBar;

    std::optional<SelectionRect> selection_rect;
    std::optional<ClipDragState> drag_state;
    bool is_wrong = false;
    float_t last_pinch_dist = 0.0f;

    void drawLayers(MTimeline &timeline, QPainter &p, QRect &r);
    void drawClip(size_t layer_idx, MClip clip, QPainter &p, QRect &r);
    void drawPlayhead(int64_t playhead_frame, QPainter &p, QRect &r);
    void drawRuler(QPainter &p, QRect &r);
    void drawSelectionRect(QPainter &p, QRect &r);
};
