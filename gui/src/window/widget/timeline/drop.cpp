#include "drop.h"
#include "TimelineCanvasWidget.h"
#include "ffi/ClientState.h"
#include "ffi/project/Project.h"
#include "ffi/project/RenderRows.h"

#include <QDropEvent>

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

bool handleVideoFileDrop(TimelineCanvasWidget *canvas, const QDropEvent *event) {
    if (!canvas || !event) {
        return false;
    }

    QStringList paths = extractDropFilePaths(event->mimeData());
    if (paths.isEmpty()) {
        return false;
    }

    QString filePath = paths.first();

    // ドロップ位置からフレームとレイヤーを算出
    QPoint localPos = event->position().toPoint();
    int64_t frame = canvas->XToFrame(localPos.x());
    int rowIdx = canvas->YToRow(localPos.y());

    uint64_t layerId = 0;
    bool canAdd = false;

    {
        auto projectResult = canvas->windowState.state->getProject();
        if (projectResult.isError()) {
            return false;
        }
        auto project = std::move(projectResult.unwrap());

        canvas->updateSnapshot();
        auto rr = canvas->cachedRows.get();
        if (!rr) {
            return false;
        }

        const auto &rows = rr->rows();
        if (rowIdx >= 0 && static_cast<size_t>(rowIdx) < rows.size()) {
            const auto &row = rows[rowIdx];
            if (row.timeline_id == canvas->timelineId) {
                layerId = row.node_id;
                canAdd = true;
            }
        }
    }

    if (canAdd) {
        canvas->commandQueue
            .addVideoClipAt(canvas->timelineId, frame, layerId, filePath.toStdString())
            .unwrap();
        canvas->markRowsDirty();
        canvas->update();
        return true;
    }

    return false;
}

} // namespace esotereel::window
