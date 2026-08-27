#pragma once

#include <cmath>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <qpair.h>
#include <qpoint.h>
#include <set>
#include <variant>

#include <QBrush>
#include <QColor>
#include <QElapsedTimer>
#include <QEvent>
#include <QLine>
#include <QMenu>
#include <QPainter>
#include <QScrollBar>
#include <QTimer>
#include <QVariant>
#include <QWidget>

#include "window/MainWindow.h"
#include "wrapper/project/Clip.h"
#include "wrapper/project/Layer.h"
#include "wrapper/project/Project.h"
#include "wrapper/project/RenderRows.h"
#include "wrapper/project/Timeline.h"

#include "esotereel_gui_helper.h"

namespace esotereel::window {
struct WindowGState;

constexpr qreal CLIP_ROUND_RADIUS = 3;
constexpr int RULER_STEP = 10;

constexpr float_t INDENT_WIDTH = 12.0;
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
using TimelineId = esotereel_gui_helper::TimelineId;
using TimelineTick = esotereel_gui_helper::TimelineTick;

class TimelineWidget : public QWidget {
    Q_OBJECT

  public:
    TimelineId timelineIdx;
    float_t zoom = 4;
    QPointF scroll = QPointF();
    int64_t playhead = 0;
    std::set<uint64_t> selectedClipIds; // clipid

    explicit TimelineWidget(WindowGState &windowState, size_t timelineIdx);
    ~TimelineWidget();

    double_t frameToX(int64_t frame) const noexcept {
        return frame * this->zoom - this->scroll.x() + LABEL_WIDTH;
    }

    int64_t XToFrame(double_t x) const noexcept {
        return std::floor((x - LABEL_WIDTH + this->scroll.x()) / this->zoom);
    }

    double_t rowToY(size_t rowIdx) const noexcept {
        return rowIdx * LAYER_HEIGHT + RULER_HEIGHT - this->scroll.y();
    }

    int YToRow(double_t y) const noexcept {
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

    Timeline getTimeline(Project &project);
    void markRowsDirty();

  protected:
    void paintEvent(QPaintEvent *e) override;
    void resizeEvent(QResizeEvent *e) override;
    void mousePressEvent(QMouseEvent *e) override;
    void mouseReleaseEvent(QMouseEvent *e) override;
    void mouseMoveEvent(QMouseEvent *e) override;
    void wheelEvent(QWheelEvent *e) override;
    void mouseDoubleClickEvent(QMouseEvent *e) override;
    void contextMenuEvent(QContextMenuEvent *e) override;
    void keyPressEvent(QKeyEvent *e) override;
    bool event(QEvent *e) override;

  private:
    // objects
    QScrollBar *hScrollBar;
    QScrollBar *vScrollBar;
    QTimer *playbackTimer;
    QElapsedTimer playbackElapsedTimer;

    // states
    DragState dragState = DragNone{};
    WindowGState &windowState;
    std::optional<QPoint> firstClickPos = std::nullopt;
    float_t last_pinch_dist = 0.0f;
    int64_t playbackStartFrame = 0;
    bool isPlaying = false;
    std::vector<uint64_t> openCompositeIds;
    std::vector<uint64_t> openFolderIds; // 開いているフォルダー(Layer)のid
    mutable std::unique_ptr<RenderRows> cachedRows;
    mutable bool rowsDirty = true;
    mutable bool fetchPending = false;

    // functions
    QColor getLabelBgColor() const noexcept;
    QRect getInnerRect() const noexcept;
    std::tuple<Clip, uint64_t> findClipAt(const Project &project, const QPoint &local) const;
    void updateSnapshot() const;

    void drawLayers(const Project &project, QPainter &p, const QRect &r) const;
    QColor rowContentBackgroundColor(size_t rowIdx) const noexcept;
    void drawRowLabel(const Project &project, const FfiLayerRow &row, QPainter &p, const QRect &r, double_t y) const;
    void drawClip(const ClipRenderInfo &info, QPainter &p, const QRect &r, double_t y) const;
    void drawPlayhead(const int64_t playhead_frame, QPainter &p, const QRect &r) const;
    void drawRuler(QPainter &p, const QRect &r) const;
    void drawSelectionRect(QPainter &p, const QRect &r) const;
    void drawDragGhost(const Project &project, QPainter &p, const QRect &r) const;

    std::optional<DragClip> handleClipDragGrab(const Project &project, const QPoint &local, bool ctrl);
    void handleClipDragContinue(const Project &project, const QPoint &local);
    void handleClipDraggingDrop(const Project &project, const QPoint &local);
    // 選択中の全クリップを、指定した移動量(frameMoved/layerMoved)の位置へ
    // 配置できるかを判定する。handleClipDragContinue(仮置きの可否判定)と
    // drawDragGhost(ゴースト描画の色分け)で共有するロジック。
    bool canDropSelectedClipsAt(const Timeline &timeline, int64_t frameMoved, int32_t layerMoved) const;

    bool handleSelectClip(const Project &project, const QPoint &mousePos, bool ctrl);
    std::optional<DragAreaSel> handleAreaSelStart(const QPoint &mousePos, bool ctrl);
    void handleAreaSelContinue(const QPoint &mousePos);
    void handleAreaSelEnd(const Project &project);

    DragState onDragStarted(QMouseEvent *e, QPoint firstClickPos);
    void onDragContinue(QMouseEvent *e);
    void onDragEnd(QMouseEvent *e);

    void handleCtrlPlayhead(const QPoint &mousePos);
    void checkEdgeScroll(const QPoint &mousePos, const QRect &r);

    void debugProjectLog();
    void togglePlayback();
    void addClipAt(const QPoint &local);
    void toggleComposite(uint64_t clipId);
    void toggleFolder(uint64_t layerId);
    void openFolder(uint64_t layerId);
    bool handleFolderLabelClick(const Project &project, const QPoint &local);
    void buildLayerContextMenu(const Project &project, QMenu &menu, const QPoint &local);
    void addLayer(std::optional<uint64_t> parentLayerId, std::optional<uint32_t> insertIndex, bool isFolder);

    void requestFrameFetch();
    void processPendingFetch();
    // 再生タイマーのtick毎に呼ばれる。経過時間からplayheadを進める。
    void advancePlaybackFrame();

    std::pair<TimelineTick, TimelineTick> getVisibleFrameRange() const noexcept;
};
} // namespace esotereel::window