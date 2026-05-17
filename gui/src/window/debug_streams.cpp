#include "debug_streams.h"
#include "../wrapper/network.h"
#include "esotereel_gui_helper.h"
#include <QPainter>
#include <QTimer>

DebugStreamsWidget::DebugStreamsWidget(WindowGState *windowState, QWidget *parent)
    : windowState(windowState), QWidget(parent), hScrollBar(nullptr) {
    // フローティング時に消えないよう最小サイズを設定
    setMinimumSize(320, 240);
}

void DebugStreamsWidget::showEvent(QShowEvent *event) {

    renderTimer = new QTimer(this);
    connect(renderTimer, &QTimer::timeout, this, [this]() { update(); });
    renderTimer->start(16);
}

void DebugStreamsWidget::paintEvent(QPaintEvent *e) {
    QWidget::paintEvent(e);

    this->updateMap();

    QPainter p(this);
    QRect r = rect();
    // 背景
    p.fillRect(r, palette().window());

    const int margin = 40;
    const double pixelsPerSecond = 60;

    for (auto &[resId, streams] : streamMap) {
        int timelineY = resId * margin;

        for (double ts : streams) {
            int x = static_cast<int>(ts * pixelsPerSecond);
            p.drawLine(x, timelineY - 15, x, timelineY + 15);
        }
    }
}

void DebugStreamsWidget::updateMap() {
    const esotereel_gui_helper::ClientNetworkHandler *network = *windowState->network;

    size_t resourceCount = esotereel_gui_helper::debug_streams_get_resources_arr_size(network);

    std::vector<uint32_t> resources(resourceCount);

    if (resourceCount > 0) {
        if (!esotereel_gui_helper::debug_streams_write_resources_arr(network, resources.data(), resources.size())) {
            return;
        }
    }
    streamMap.clear();

    for (uint32_t resourceId : resources) {
        size_t secCount = esotereel_gui_helper::debug_streams_get_loaded_streams_sec_arr_size(network, resourceId);

        std::vector<double> secs(secCount);

        if (secCount > 0) {
            if (!esotereel_gui_helper::debug_streams_write_loaded_streams_sec_arr(network, resourceId, secs.data(),
                                                                                  secs.size())) {
                continue;
            }
        }

        streamMap.emplace(resourceId, std::move(secs));
    }
}