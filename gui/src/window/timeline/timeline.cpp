#include "timeline.h"
#include "wrapper/network.h"
#include "wrapper/project/clip.h"
#include "wrapper/requests.h"
#include <tuple>

TimelineWidget::TimelineWidget(WindowGState &windowState, size_t timelineIdx)
    : windowState(windowState), timelineIdx(timelineIdx) {
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
        this->advancePlaybackFrame();
    });
}

void TimelineWidget::advancePlaybackFrame() {
    // プロジェクトのFPS（とりあえず60固定と想定）
    const double fps = 60.0;

    // 再生開始時からの経過時間を取得 (ms)
    qint64 elapsedMs = this->playbackElapsedTimer.elapsed();

    // 現在のフレームを算出
    this->playhead = this->playbackStartFrame + static_cast<int64_t>(elapsedMs * fps / 1000.0);

    this->requestFrameFetch();
    update();
}

TimelineWidget::~TimelineWidget() = default;

void TimelineWidget::resizeEvent(QResizeEvent *event) {
    QWidget::resizeEvent(event);

    int sw = SCROLLBAR_SIZE;
    hScrollBar->setGeometry(0, height() - sw, width() - sw, sw);
    vScrollBar->setGeometry(width() - sw, 0, sw, height() - sw);
}

std::tuple<Clip, uint64_t> TimelineWidget::findClipAt(const Project &project, const QPoint &local) const {
    // 1. ルーラー領域、またはレイヤーラベル領域へのクリックは対象外
    if (local.x() < LABEL_WIDTH || local.y() < RULER_HEIGHT) {
        return std::make_tuple(Clip::Empty(), 0);
    }

    int64_t frame = this->XToFrame(local.x());
    size_t rowIdx = this->YToRow(local.y());

    // 2. 展開済みの階層行情報（RenderRows）を取得

    std::unique_ptr<RenderRows> &rr = this->cachedRows;
    if (!rr) {
        return std::make_tuple(Clip::Empty(), 0);
    }
    auto rows = rr->rows();

    // 範囲チェック（画面上の行インデックスが実際の行数を超えている場合）
    if (rowIdx >= rows.size()) {
        return std::make_tuple(Clip::Empty(), 0);
    }

    const FfiLayerRow &row = rows[rowIdx];
    auto clips = rr->clipsFor(row);

    // 3. 該当行の中にあるクリップから、クリックされたフレームに位置するものを探す
    for (const auto &clipInfo : clips) {
        int64_t clipStart = clipInfo.abs_frame;
        int64_t clipEnd = clipInfo.abs_frame + clipInfo.duration;

        if (frame >= clipStart && frame < clipEnd) {
            auto timeline = project.timelineOf(this->timelineIdx);
            auto [clip, layerId] = timeline.findClipById(clipInfo.clip_id);

            if (clip.isValid()) {
                return std::make_tuple(clip, layerId);
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

void TimelineWidget::updateSnapshot() const {
    if (!rowsDirty && cachedRows)
        return;

    // スコープ内でロックを取得し、RenderRowsを構築したら即座にロックを破棄
    {
        auto projectResult = windowState.network->getProject();
        if (projectResult.isError()) {
            return;
        }
        auto project = projectResult.unwrapOrMove();

        cachedRows = std::make_unique<RenderRows>(project, timelineIdx, openCompositeIds, openFolderIds);
    } // <- ここで Project のデストラクタが走り ReadGuard 解放

    rowsDirty = false;
}

namespace {
// idがidsに含まれていれば取り除き、なければ追加する(開閉トグルの共通処理)。
void toggleId(std::vector<uint64_t> &ids, uint64_t id) {
    auto it = std::find(ids.begin(), ids.end(), id);
    if (it != ids.end()) {
        ids.erase(it);
    } else {
        ids.push_back(id);
    }
}
} // namespace

void TimelineWidget::toggleComposite(uint64_t clipId) {
    toggleId(openCompositeIds, clipId);
    rowsDirty = true;
    update();
}

void TimelineWidget::toggleFolder(uint64_t layerId) {
    toggleId(openFolderIds, layerId);
    rowsDirty = true;
    update();
}

// トグルではなく「必ず開く」。フォルダーに新規レイヤーを追加した直後、
// 追加先が見えるように呼ぶ。
void TimelineWidget::openFolder(uint64_t layerId) {
    if (std::find(openFolderIds.begin(), openFolderIds.end(), layerId) == openFolderIds.end()) {
        openFolderIds.push_back(layerId);
        rowsDirty = true;
        update();
    }
}

// ラベル領域(フォルダーの▶▼部分含む)がクリックされたときの開閉トグル。
// フォルダー行なら true を返し、呼び出し側はドラッグ開始等を行わない。
bool TimelineWidget::handleFolderLabelClick(const Project &project, const QPoint &local) {
    if (local.x() >= LABEL_WIDTH || local.y() < RULER_HEIGHT) {
        return false;
    }
    if (!project.isValid()) {
        return false;
    }

    std::unique_ptr<RenderRows> &rr = this->cachedRows;
    if (!rr) {
        return false;
    }

    const auto &rows = rr->rows();
    int rowIdx = this->YToRow(local.y());
    if (rowIdx < 0 || static_cast<size_t>(rowIdx) >= rows.size()) {
        return false;
    }

    const FfiLayerRow &row = rows[rowIdx];
    if (!row.is_folder) {
        return false;
    }

    this->toggleFolder(row.layer_id);
    return true;
}

void TimelineWidget::markRowsDirty() {
    rowsDirty = true;
    update();
}

void TimelineWidget::requestFrameFetch() {
    this->fetchPending = true;

    QMetaObject::invokeMethod(this, &TimelineWidget::processPendingFetch, Qt::QueuedConnection);
}

void TimelineWidget::processPendingFetch() {
    if (!this->fetchPending) {
        return;
    }
    this->fetchPending = false;

    {
        auto projectResult = windowState.network->getProject();
        if (projectResult.isError())
            return;
    }

    // 可視領域のフレーム範囲を算出
    auto visible = getVisibleFrameRange();

    // Wrapper 経由で FFI (req_fetch_frame) を実行
    this->windowState.network->requests().fetchFrame(this->timelineIdx, this->playhead, visible);
}

std::pair<Tick, Tick> TimelineWidget::getVisibleFrameRange() const noexcept {
    const QRect r = getInnerRect();
    // ラベル分を除いた実際の描画開始x座標から終了x座標まで
    Tick startFrame = XToFrame(r.left());
    Tick endFrame = XToFrame(r.right());

    // 念のため下限をクランプ（負のフレームを送らないように）
    startFrame = std::max((Tick)0, startFrame);
    endFrame = std::max(startFrame, endFrame);

    return {startFrame, endFrame};
}