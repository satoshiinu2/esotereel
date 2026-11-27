#include "drop.h"
#include "TimelineCanvasWidget.h"
#include "ffi/ClientState.h"
#include "ffi/project/Project.h"
#include "ffi/project/RenderRows.h"
#include "window/widget/assets/AssetsPanel.h"

#include <QDropEvent>
#include <algorithm>
#include <array>

namespace esotereel::window {

QStringList extractDropFilePaths(const QMimeData *mimeData) {
    QStringList paths;
    if (mimeData && mimeData->hasUrls()) {
        for (const QUrl &url : mimeData->urls()) {
            if (url.isLocalFile()) {
                paths.append(url.toLocalFile());
            }
        }
    }
    return paths;
}

std::optional<std::pair<TimelineTick, LayerId>>
TimelineCanvasWidget::resolveDropTarget(const QPoint &localPos) {
    const int64_t frame = this->XToFrame(localPos.x());
    const int rowIdx = this->YToRow(localPos.y());

    // updateSnapshot() は内部で getProject() して自身のガードを取得・解放
    // する自己完結した関数なので、Project ガードを取得する前に呼ぶ。
    // ガード保持中に呼ぶと同一 RwLock の read を入れ子に取得することに
    // なり、間に write がキューされるとデッドロックする。
    // 行データは読み取りの前に最新化しておく必要があるため、この順序を保つ。
    this->updateSnapshot();

    auto projectResult = this->windowState.state->getProject();
    if (projectResult.isError()) {
        return std::nullopt;
    }
    auto project = std::move(projectResult.unwrap());

    auto rr = this->cachedRows.get();
    if (!rr) {
        return std::nullopt;
    }

    const auto &rows = rr->rows();
    if (rowIdx < 0 || static_cast<size_t>(rowIdx) >= rows.size()) {
        return std::nullopt;
    }
    const auto &row = rows[rowIdx];
    if (row.timeline_id != this->timelineId) {
        return std::nullopt;
    }
    return std::make_pair(frame, row.node_id);
}

bool handleVideoFileDrop(TimelineCanvasWidget *canvas, const QDropEvent *event) {
    if (!canvas || !event) {
        return false;
    }

    QStringList paths = extractDropFilePaths(event->mimeData());
    if (paths.isEmpty()) {
        return false;
    }

    QString filePath = paths.first();

    const auto target = canvas->resolveDropTarget(event->position().toPoint());
    if (!target) {
        return false;
    }

    auto addResult = canvas->commandQueue.addVideoClipAt(canvas->timelineId, target->first, target->second,
                                                         filePath.toStdString());
    if (addResult.isError()) {
        auto err = addResult.getError();
        qWarning() << "TimelineCanvasWidget: addVideoClipAt failed:" << QString::fromStdString(*err);
        return false;
    }
    canvas->markRowsDirty();
    canvas->update();
    return true;
}

bool handleAssetDrop(TimelineCanvasWidget *canvas, const QDropEvent *event) {
    if (!canvas || !event) {
        return false;
    }

    const QByteArray data = event->mimeData()->data(QString::fromLatin1(kAssetRefMimeType));
    if (data.size() != 16) {
        return false;
    }
    std::array<uint8_t, 16> assetId{};
    std::copy(data.begin(), data.end(), assetId.begin());

    const auto target = canvas->resolveDropTarget(event->position().toPoint());
    if (!target) {
        return false;
    }

    auto addResult = canvas->commandQueue.addClipFromAssetAt(canvas->timelineId, target->first, target->second,
                                                             assetId);
    if (addResult.isError()) {
        auto err = addResult.getError();
        qWarning() << "TimelineCanvasWidget: addClipFromAssetAt failed:" << QString::fromStdString(*err);
        return false;
    }
    canvas->markRowsDirty();
    canvas->update();
    return true;
}

} // namespace esotereel::window
