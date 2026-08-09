

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

std::tuple<Clip, size_t> TimelineWidget::findClipAt(const Project &project, const Timeline &timeline,
                                                    const QPoint &local) const {
    // 1. ルーラー領域、またはレイヤーラベル領域へのクリックは対象外
    if (local.x() < LABEL_WIDTH || local.y() < RULER_HEIGHT) {
        return std::make_tuple(Clip::Empty(), 0);
    }

    int64_t frame = this->XToFrame(local.x());
    size_t rowIdx = this->YToRow(local.y());

    // 2. 展開済みの階層行情報（RenderRows）を取得
    const RenderRows &renderRows = this->getRows(project, timeline);
    auto rows = renderRows.rows();

    // 範囲チェック（画面上の行インデックスが実際の行数を超えている場合）
    if (rowIdx >= rows.size()) {
        return std::make_tuple(Clip::Empty(), 0);
    }

    const FfiLayerRow &row = rows[rowIdx];
    auto clips = renderRows.clipsFor(row);

    // 3. 該当行の中にあるクリップから、クリックされたフレームに位置するものを探す
    for (const auto &clipInfo : clips) {
        int64_t clipStart = clipInfo.abs_frame;
        int64_t clipEnd = clipInfo.abs_frame + clipInfo.duration;

        if (frame >= clipStart && frame < clipEnd) {
            auto [clip, _layer] = timeline.findClipById(clipInfo.clip_id);

            if (clip.isValid()) {
                return std::make_tuple(clip, rowIdx);
            }
        }
    }

    return std::make_tuple(Clip::Empty(), 0);
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