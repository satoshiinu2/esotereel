#include "debug_streams.h"
#include "../wrapper/network.h"
#include "esotereel_gui_helper.h"
#include <QPainter>
#include <QResizeEvent>
#include <QTimer>

DebugStreamsWidget::DebugStreamsWidget(WindowGState *windowState, QWidget *parent)
    : windowState(windowState), QWidget(parent) {
    // フローティング時に消えないよう最小サイズを設定
    setMinimumSize(320, 240);

    hScrollBar = new QScrollBar(Qt::Horizontal, this);
    connect(hScrollBar, &QScrollBar::valueChanged, this, [this](int) { update(); });
}

void DebugStreamsWidget::showEvent(QShowEvent *event) {
    if (!renderTimer) {
        renderTimer = new QTimer(this);
        connect(renderTimer, &QTimer::timeout, this, [this]() {
            this->updateMap();
            this->update();
        });
        renderTimer->start(16);
    }
}

void DebugStreamsWidget::resizeEvent(QResizeEvent *event) {
    QWidget::resizeEvent(event);
    int sbHeight = hScrollBar->sizeHint().height();
    hScrollBar->setGeometry(0, height() - sbHeight, width(), sbHeight);
}

void DebugStreamsWidget::paintEvent(QPaintEvent *e) {
    QPainter p(this);
    QRect r = rect();
    // 背景
    p.fillRect(r, palette().window());

    const int margin = 40;
    const int verticalOffset = 80; // ルーラーと余白のためにオフセットを増やす
    const double pixelsPerSecond = 60;

    p.save();
    // スクロールバーの領域を除外するようにクリッピング
    QRect clipRect = r;
    clipRect.setHeight(r.height() - hScrollBar->height());
    p.setClipRect(clipRect);

    p.translate(-hScrollBar->value(), verticalOffset);

    // ルーラー（時間軸）の描画
    p.setPen(palette().text().color());
    int startX = hScrollBar->value();
    int endX = startX + width();
    int firstSec = startX / (int)pixelsPerSecond;
    int lastSec = endX / (int)pixelsPerSecond + 1;

    for (int s = firstSec; s <= lastSec; ++s) {
        int x = s * (int)pixelsPerSecond;
        // 秒の大きな目盛り
        p.setPen(QPen(palette().text().color(), 1));
        p.drawLine(x, -50, x, -30);
        // 秒とフレーム数を表示
        p.drawText(x + 3, -35, QString("%1s").arg(s*pixelsPerSecond));

        // 10フレームごとの小さな目盛り
        p.setPen(palette().mid().color());
        for (int f = 1; f < 6; ++f) {
            int fx = x + (f * 10);
            p.drawLine(fx, -40, fx, -35);
        }
    }

    int row = 0;
    for (auto &[resId, streams] : streamMap) {
        int timelineY = row * margin; // IDそのものではなく、行番号で縦位置を決定

        // リソースIDの表示（行の左端付近）
        p.setPen(palette().text().color());
        p.drawText(startX + 5, timelineY - 18, QString("Res: %1").arg(resId));

        // ベースライン（横線）を描画して、ストリームの存在をわかりやすくする
        p.setPen(QPen(palette().mid().color(), 1, Qt::DashLine));
        p.drawLine(startX, timelineY, endX, timelineY);

        // ストリームのパルスを描画
        p.setPen(QPen(palette().highlight().color(), 1)); 
        for (double ts : streams) {
            int x = static_cast<int>(ts * pixelsPerSecond);
            p.drawLine(x, timelineY - 15, x, timelineY + 15);
        }
        row++;
    }
    p.restore();
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

    // 最大タイムスタンプからスクロール範囲を計算
    double maxTs = 0;
    for (auto const &[resId, streams] : streamMap) {
        if (!streams.empty()) {
            auto ts = streams.size();
            if (ts > maxTs) {
                maxTs = ts;
            }
        }
    }

    const double pixelsPerSecond = 60;
    int contentWidth = static_cast<int>(maxTs * pixelsPerSecond) + 100; // 余白を追加   if (maxTs > 0) {
    int newRange = std::max(0, contentWidth - width());
    if (hScrollBar->maximum() != newRange) {
        hScrollBar->setPageStep(width());
        hScrollBar->setRange(0, newRange);
    }
}
