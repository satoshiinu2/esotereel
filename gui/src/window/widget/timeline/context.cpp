#include "TimelineWidget.h"
#include "ffi/ClientNetworkHandler.h"
#include "ffi/Requests.h"

#include <QContextMenuEvent>
#include <QEvent>
#include <QInputDialog>
#include <QLineEdit>
#include <QMenu>
#include <optional>
#include <span>

namespace esotereel::window {
// rowIdx の行の「親レイヤー」を RenderRows の depth だけから逆算する。
// フォルダーが開いているときだけ子行がdepth+1で並ぶ構造を利用し、
// 自分より1つ浅いdepthの直近の行(=それを開いた親フォルダー)を辿る。
// ルート直下(depth == 0)なら親なし(nullopt)。
static std::optional<uint64_t> findParentLayerId(std::span<const FfiLayerRow> rows, int rowIdx) {
    if (rowIdx < 0 || static_cast<size_t>(rowIdx) >= rows.size()) {
        return std::nullopt;
    }
    uint32_t depth = rows[rowIdx].depth;
    if (depth == 0) {
        return std::nullopt;
    }
    for (int i = rowIdx - 1; i >= 0; --i) {
        if (rows[i].depth == depth - 1) {
            return rows[i].layer_id;
        }
    }
    return std::nullopt;
}

// rowIdx の行が、同じ親の子リスト(root_layersまたはLayer.children)の中で
// 何番目か(0-indexed)を、depthだけを見て逆算する。
// 自分と同じdepthの行を数え、depthが浅い行(=親)に当たったら打ち切り。
// depthが深い行(=子孫のFolder展開/Composite展開ブロック)はカウントせず読み飛ばす。
// これにより「兄弟」だけを正しく数えられる(build_layer_row_recursiveが
// 兄弟同士を必ず連続して並べる実装になっているため成立する)。
static uint32_t siblingIndexOf(std::span<const FfiLayerRow> rows, int rowIdx) {
    uint32_t depth = rows[rowIdx].depth;
    uint32_t count = 0;
    for (int i = rowIdx - 1; i >= 0; --i) {
        if (rows[i].depth < depth) {
            break; // 親行に到達 → ここで打ち切り
        }
        if (rows[i].depth == depth) {
            count++;
        }
        // rows[i].depth > depth の行(子孫ブロック)はスキップしてそのまま遡る
    }
    return count;
}

void TimelineWidget::contextMenuEvent(QContextMenuEvent *e) {
    this->updateSnapshot();

    QPoint pos = e->pos();
    QMenu menu(this);

    // ロック保持のスコープを限定
    {
        auto projectResult = windowState.network->getProject();
        if (projectResult.isError()) {
            menu.exec(e->globalPos());
            return;
        }
        auto project = projectResult.unwrapOrMove();

        if (pos.x() < LABEL_WIDTH && pos.y() >= RULER_HEIGHT) {
            this->buildLayerContextMenu(project, menu, pos);
        } else {
            auto [clip, layerId] = this->findClipAt(project, pos);
            if (clip.isValid()) {
                QAction *clipCopyAction = menu.addAction("Copy");
                QObject::connect(clipCopyAction, &QAction::triggered, this, []() {});

                QAction *clipDeleteAction = menu.addAction("Delete");
                QObject::connect(clipDeleteAction, &QAction::triggered, this, []() {});
            } else {
                QAction *clipAddAction = menu.addAction("Add clip");
                QObject::connect(clipAddAction, &QAction::triggered, this, [pos, this]() { this->addClipAt(pos); });

                QAction *clipTestAction = menu.addAction("request test");
                QObject::connect(clipTestAction, &QAction::triggered, this, [this]() { this->debugProjectLog(); });
            }
        }
    } // <-- ここでProjectのデストラクタが呼ばれ、ロックが確実に解放される

    // ロックを解放してからメニューを表示
    if (!menu.actions().isEmpty()) {
        menu.exec(e->globalPos());
    }
}

// ラベル領域用: 右クリックした行に応じて「子として追加」か「兄弟として追加」かを決め、
// Add Layer / Add Folder のアクションを積む。
void TimelineWidget::buildLayerContextMenu(const Project &project, QMenu &menu, const QPoint &local) {
    if (!project.isValid()) {
        return;
    }

    std::unique_ptr<RenderRows> &rr = this->cachedRows;
    if (!rr) {
        return;
    }

    const auto &rows = rr->rows();
    int rowIdx = this->YToRow(local.y());

    std::optional<uint64_t> parentLayerId = std::nullopt;
    std::optional<uint32_t> insertIndex = std::nullopt;

    if (rowIdx >= 0 && static_cast<size_t>(rowIdx) < rows.size()) {
        const FfiLayerRow &row = rows[rowIdx];

        // Composite展開行(子Timeline)の上でのレイヤー追加は今回未対応。
        // ルートTimeline(このウィジェットが表示しているtimelineIdx)の行のみ許可。
        if (row.timeline_id != this->timelineId) {
            return;
        }

        if (row.is_folder) {
            // フォルダー行 → その子として追加(末尾に追加。挿入位置は指定しない)
            parentLayerId = row.layer_id;
            insertIndex = std::nullopt;
        } else {
            // 通常レイヤー行 → 同じ親(=兄弟)として、その行の直後に追加
            parentLayerId = findParentLayerId(rows, rowIdx);
            insertIndex = siblingIndexOf(rows, rowIdx) + 1;
        }
    }
    // rowIdx が範囲外(ラベル領域の空白部分)なら parentLayerId/insertIndex は
    // nullopt のまま → ルート直下(root_layers)の末尾に追加される

    QAction *addLayerAction = menu.addAction("Add Layer");
    QObject::connect(addLayerAction, &QAction::triggered, this,
                     [this, parentLayerId, insertIndex]() { this->addLayer(parentLayerId, insertIndex, false); });

    QAction *addFolderAction = menu.addAction("Add Folder");
    QObject::connect(addFolderAction, &QAction::triggered, this,
                     [this, parentLayerId, insertIndex]() { this->addLayer(parentLayerId, insertIndex, true); });
}

// 名前を聞いてからサーバーへレイヤー追加リクエストを送る。
// TODO: wrapper/requests.h に
//   addLayer(timelineId, hasParent, parentLayerId, hasIndex, index, name, isFolder)
// を追加してください(addClipAt / moveClips と同じパターンで、Rust側は
// Project::insert_layer_in_timeline(timeline_id, parent, index, name, is_folder) を
// 呼ぶハンドラを想定しています。index は Option<usize> なので hasIndex/index の
// ペアでFFI境界を越す必要があります)。
void TimelineWidget::addLayer(std::optional<uint64_t> parentLayerId, std::optional<uint32_t> insertIndex,
                              bool isFolder) {
    bool ok = false;
    const QString title = isFolder ? "Add Folder" : "Add Layer";
    const QString defaultName = isFolder ? "Folder" : "Layer";

    QString name = QInputDialog::getText(this, title, "Name:", QLineEdit::Normal, defaultName, &ok);
    if (!ok || name.trimmed().isEmpty()) {
        return;
    }

    this->windowState.network->requests().addLayer(this->timelineId, parentLayerId, insertIndex, name.toStdString(),
                                                   isFolder);

    // 子として追加した場合は、追加直後にそのフォルダーが見えるようにしておく
    if (parentLayerId.has_value()) {
        this->openFolder(parentLayerId.value());
    }

    this->markRowsDirty();
    update();
}

void TimelineWidget::addClipAt(const QPoint &local) {
    int64_t frame = 0;
    uint64_t layerId = 0;
    bool canAdd = false;

    // スコープでロック期間を最小化
    {
        auto projectResult = windowState.network->getProject();
        if (projectResult.isError())
            return;
        auto project = projectResult.unwrapOrMove();
        if (!project.isValid())
            return;

        frame = this->XToFrame(local.x());
        int rowIdx = this->YToRow(local.y());

        std::unique_ptr<RenderRows> &rr = this->cachedRows;
        if (!rr) {
            return;
        }
        const auto &rows = rr->rows();
        if (rowIdx >= 0 && static_cast<size_t>(rowIdx) < rows.size()) {
            const auto &row = rows[rowIdx];
            if (row.timeline_id == this->timelineId) {
                layerId = row.layer_id;
                canAdd = true;
            }
        }
    } // <-- ロック解放

    if (canAdd) {
        // ロックを持たない状態でネットワークへリクエスト
        this->windowState.network->requests().addClipAt(this->timelineId, frame, layerId);
        this->markRowsDirty();
        update();
    }
}

void TimelineWidget::debugProjectLog() {
    this->windowState.network->requests().debugProjectLog();
}
} // namespace esotereel::window