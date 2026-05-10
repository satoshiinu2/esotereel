#pragma once

#include "../wrapper/project/forwards.h"
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
#include <variant>

struct WindowGState;

constexpr qreal CLIP_ROUND_RADIUS = 3;
constexpr int RULER_STEP = 10;

constexpr float_t LAYER_HEIGHT = 32.0;
constexpr float_t RULER_HEIGHT = 24.0;
constexpr float_t LABEL_WIDTH = 80.0;
constexpr float_t SCROLLBAR_SIZE = 12.0;

constexpr double_t DEFAULT_FRAME_COUNT = 300;
constexpr double_t DEFAULT_LAYER_LEN = 1;

struct DragNone {};
struct DragOther {};

struct DragClip {
    size_t srcLayerIdx;
    int64_t srcFrame;
    size_t curLayerIdx;
    int64_t curFrame;
    QPointF ghostPos;
    bool isWrong;
};

struct DragAreaSel {
    QPointF start;
    QPointF current;
};

struct DragPlayHead {};

using DragState = std::variant<DragNone, DragOther, DragClip, DragAreaSel, DragPlayHead>;

class TimelineWidget : public QWidget {
    Q_OBJECT

  public:
    size_t timelineIdx;
    float_t zoom = 4;
    QPointF scroll = QPointF();
    int64_t playhead = 0;
    std::set<uint64_t> selectedClipIds; // clipid

    TimelineWidget(WindowGState *windowState, size_t timelineType);

    double_t frameToX(int64_t frame) const noexcept {
        return frame * this->zoom - this->scroll.x() + LABEL_WIDTH;
    }

    int64_t XToFrame(double_t x) const noexcept {
        return std::floor((x - LABEL_WIDTH + this->scroll.x()) / this->zoom);
    }

    double_t layerToY(size_t layer_idx) const noexcept {
        return layer_idx * LAYER_HEIGHT + RULER_HEIGHT - this->scroll.y();
    }

    int YToLayerIdx(double_t y) const noexcept {
        return std::floor((y - RULER_HEIGHT + this->scroll.y()) / LAYER_HEIGHT);
    }

    void setScrollX(qreal x) {
        this->scroll.setX(x);
        hScrollBar->setValue(x);
    }
    void setScrollY(qreal y) {
        this->scroll.setY(y);
        vScrollBar->setValue(y);
    }

    Timeline getTimeline(Project &project) ;

  protected:
    void paintEvent(QPaintEvent *e) override;
    void resizeEvent(QResizeEvent *e) override;
    void mousePressEvent(QMouseEvent *e) override;
    void mouseReleaseEvent(QMouseEvent *e) override;
    void mouseMoveEvent(QMouseEvent *e) override;
    void wheelEvent(QWheelEvent *e) override;
    void mouseDoubleClickEvent(QMouseEvent *e) override;
    void contextMenuEvent(QContextMenuEvent *e) override;

  private:
    QScrollBar *hScrollBar;
    QScrollBar *vScrollBar;

    DragState dragState = DragNone{};
    WindowGState *windowState;
    std::optional<QPoint> firstClickPos = std::nullopt;
    float_t last_pinch_dist = 0.0f;

    QRect getInnerRect() const noexcept;
    std::tuple<Clip, size_t> findClipAt(const Timeline &timeline, const QPoint &local) const;

    void drawLayers(const Timeline &timeline, QPainter &p, const QRect &r) const;
    void drawClip(size_t layer_idx, const Clip &clip, QPainter &p, const QRect &r) const;
    void drawPlayhead(const int64_t playhead_frame, QPainter &p, const QRect &r) const;
    void drawRuler(QPainter &p, const QRect &r) const;
    void drawSelectionRect(QPainter &p, const QRect &r) const;
    void drawDragGhost(const Timeline &timeline, QPainter &p, const QRect &r) const;

    std::optional<DragClip> handleClipDragGrab(const Timeline &timeline, const QPoint &local, bool ctrl);
    void handleClipDragContinue(const Timeline &timeline, const QPoint &local);
    void handleClipDraggingDrop(const Timeline &timeline, const QPoint &local);

    bool handleSelectClip(Timeline &timeline, const QPoint &mousePos, bool ctrl);
    std::optional<DragAreaSel> handleAreaSelStart(const QPoint &mousePos, bool ctrl);
    void handleAreaSelContinue(const QPoint &mousePos);
    void handleAreaSelEnd(const Timeline &timeline);

    DragState onDragStarted(QMouseEvent *e, QPoint firstClickPos);
    void onDragContinue(QMouseEvent *e);
    void onDragEnd(QMouseEvent *e);

    void handleCtrlPlayhead(const QPoint &mousePos);
    void checkEdgeScroll(const QPoint &mousePos, const QRect &r);

    void addClipAt(const QPoint &local);
};
