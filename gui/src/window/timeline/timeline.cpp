

#include "../../wrapper/project/timeline.h"         // IWYU pragma: keep
#include "../../wrapper/project/clip.h"             // IWYU pragma: keep
#include "../../wrapper/project/clip_render_info.h" // IWYU pragma: keep
#include "../../wrapper/project/project.h"          // IWYU pragma: keep
#include "../../wrapper/project/timeline_layers.h"  // IWYU pragma: keep
#include "../main.h"
#include "timeline.h"
#include <QTimer>
#include <cmath>
#include <cstddef>
#include <qpoint.h>
#include <qwidget.h>
#include <tuple>

TimelineWidget::TimelineWidget(WindowGState &windowState, size_t timelineType)
    : windowState(windowState), timelineIdx(timelineType) {
    hScrollBar = new QScrollBar(Qt::Horizontal, this);
    vScrollBar = new QScrollBar(Qt::Vertical, this);

    connect(hScrollBar, &QScrollBar::valueChanged, [this](int val) {
        this->scroll.setX(val);
        update();
    });
    connect(vScrollBar, &QScrollBar::valueChanged, [this](int val) {
        this->scroll.setY(val);
        update();
    });

    // 再生用タイマーの設定
    playbackTimer = new QTimer(this);
    connect(playbackTimer, &QTimer::timeout, this, [this]() {
        if (!this->isPlaying)
            return;

        // プロジェクトのFPS（とりあえず60固定と想定）
        const double fps = 60.0;

        // 再生開始時からの経過時間を取得 (ms)
        qint64 elapsedMs = this->playbackElapsedTimer.elapsed();

        // 現在のフレームを算出
        this->playhead = this->playbackStartFrame + static_cast<int64_t>(elapsedMs * fps / 1000.0);

        update();
    });
}

TimelineWidget::~TimelineWidget() = default;

void TimelineWidget::resizeEvent(QResizeEvent *event) {
    QWidget::resizeEvent(event);

    int sw = SCROLLBAR_SIZE;
    hScrollBar->setGeometry(0, height() - sw, width() - sw, sw);
    vScrollBar->setGeometry(width() - sw, 0, sw, height() - sw);
}

std::tuple<Clip, size_t> TimelineWidget::findClipAt(const Timeline &timeline, const QPoint &local) const {
    // ルーラー領域、またはレイヤーラベル領域へのクリックは、クリップ選択の対象外とする
    if (local.x() < LABEL_WIDTH || local.y() < RULER_HEIGHT) {
        return std::make_tuple(Clip::Empty(), 0);
    }

    int64_t frame = this->XToFrame(local.x());
    size_t row = this->YToRow(local.y());

    // range check
    if (row >= timeline.layersCount()) {
        return std::make_tuple(Clip::Empty(), 0);
    }
    Layer layer = timeline.layerSortedAt(row);
    if (!layer.isValid()) {
        return std::make_tuple(Clip::Empty(), 0);
    }

    Clip clip = layer.findClipAtFrame(frame);
    if (!clip.isValid()) {
        return std::make_tuple(Clip::Empty(), 0);
    }

    return std::make_tuple(clip, row);
}

Timeline TimelineWidget::getTimeline(Project &project) {
    return project.isValid() ? project.timelineOf(this->timelineIdx) : Timeline(nullptr);
}

void TimelineWidget::togglePlayback() {
    if (isPlaying) {
        playbackTimer->stop();
        isPlaying = false;
    } else {
        this->playbackStartFrame = this->playhead;
        this->playbackElapsedTimer.start();
        this->playbackTimer->start(16); // 約60FPSでUI更新
        this->isPlaying = true;
    }
}

const RenderRows &TimelineWidget::getRows(const Project &project, const Timeline &timeline) const {
    if (rowsDirty || !cachedRows) {
        cachedRows = std::make_unique<RenderRows>(project, timeline, openCompositeIds);
        rowsDirty = false;
    }
    return *cachedRows;
}

void TimelineWidget::toggleComposite(uint64_t clipId) {
    auto it = std::find(openCompositeIds.begin(), openCompositeIds.end(), clipId);
    if (it != openCompositeIds.end()) {
        openCompositeIds.erase(it);
    } else {
        openCompositeIds.push_back(clipId);
    }
    rowsDirty = true;
    update();
}

void TimelineWidget::markRowsDirty() {
    rowsDirty = true;
    update();
}