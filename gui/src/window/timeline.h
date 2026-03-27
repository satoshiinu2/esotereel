#pragma once

#include "../wrapper/clip.h"
#include "../wrapper/project.h"
#include "../wrapper/timeline.h"
#include <QEvent>
#include <QPainter>
#include <QScrollBar>
#include <QWidget>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <qpair.h>
#include <qpoint.h>
#include <set>

constexpr qreal CLIP_ROUND_RADIUS = 3;
constexpr int RULER_STEP = 10;

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
    size_t srcLayerIdx;
    uint64_t srcFrame;
    size_t curLayerIdx;
    uint64_t curFrame;
    QPointF ghostPos;
    bool isWrong;
};

class TimelineWidget : public QWidget {
    Q_OBJECT

  public:
    size_t timelineType;
    float_t zoom = 4;
    QPointF scroll = QPointF();
    int64_t playhead = 0;
    std::set<uint64_t> selectedClipIds; // clipid

    TimelineWidget(size_t timelineType);

    double_t frameToX(double_t frame) const noexcept {
        return frame * this->zoom - this->scroll.x() + LABEL_WIDTH;
    }

    uint64_t XToFrame(double_t x) const noexcept {
        return ((x - LABEL_WIDTH + this->scroll.x()) / this->zoom);
    }

    double_t layerToY(size_t layer_idx) const noexcept {
        return layer_idx * LAYER_HEIGHT + RULER_HEIGHT - this->scroll.y();
    }

    void setScrollX(qreal x) {
        this->scroll.setX(x);
        hScrollBar->setValue(x);
    }
    void setScrollY(qreal y) {
        this->scroll.setY(y);
        vScrollBar->setValue(y);
    }

    MTimeline getTimeline() {
        MProject project = getProject();
        return project.isValid() ? project.timelineOf(this->timelineType) : MTimeline(nullptr);
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

    std::optional<SelectionRect> selectionRect;
    std::optional<ClipDragState> dragState;
    bool isDragging = false;
    float_t last_pinch_dist = 0.0f;

    MClipLocation findClipAt(const MTimeline &timeline, const QPoint &local) const;

    void drawLayers(const MTimeline &timeline, QPainter &p, const QRect &r) const;
    void drawClip(size_t layer_idx, const MClip &clip, QPainter &p, const QRect &r) const;
    void drawPlayhead(const int64_t playhead_frame, QPainter &p, const QRect &r) const;
    void drawRuler(QPainter &p, const QRect &r) const;
    void drawSelectionRect(QPainter &p, const QRect &r) const;
    void drawDragGhost(const MTimeline &timeline, QPainter &p, const QRect &r) const;

    bool handleDragGrab(const MTimeline &timeline, const QPoint &local, bool ctrl);
    void handleDragContinue(const MTimeline &timeline, const QPoint &local);
    void handleDragDrop(const MTimeline &timeline, const QPoint &local);

    bool handleSelectClip(MTimeline &timeline, const QPoint &mousePos, bool ctrl);
    void handleAreaSelStart(const QPoint &mousePos, bool ctrl);
    void handleAreaSelContinue(const QPoint &mousePos);
    void handleAreaSelEnd(const MTimeline &timeline);

    void onDragStarted(QMouseEvent *e);
    void onDragContinue(QMouseEvent *e);
    void onDragEnd(QMouseEvent *e);

    void handleCtrlPlayhead(const QPoint &mousePos);
    void checkEdgeScroll(const QPoint &mousePos, const QRect &r);
};
