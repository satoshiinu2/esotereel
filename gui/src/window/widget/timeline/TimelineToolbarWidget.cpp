#include "TimelineToolbarWidget.h"
#include "TimelineCanvasWidget.h"
#include "ffi/Toolbar.h"
#include "window/MainWindow.h"

#include <QDebug>
#include <QToolButton>

namespace esotereel::window {

TimelineToolbarWidget::TimelineToolbarWidget(QWidget *parent) : QWidget(parent) {
    layout = new QHBoxLayout(this);
    layout->setContentsMargins(4, 2, 4, 2);
    layout->setSpacing(2);
    layout->addStretch(1); // ボタンは左詰め、右側は余白
}

const std::unordered_map<QString, std::function<void(TimelineCanvasWidget &)>> &
TimelineToolbarWidget::builtinCommands() {
    static const std::unordered_map<QString, std::function<void(TimelineCanvasWidget &)>> commands = {
        {"add_layer", [](TimelineCanvasWidget &c) { c.addLayerAtRoot(); }},
        {"add_folder", [](TimelineCanvasWidget &c) { c.addFolderAtRoot(); }},
        {"zoom_in", [](TimelineCanvasWidget &c) { c.zoomBy(1.1f); }},
        {"zoom_out", [](TimelineCanvasWidget &c) { c.zoomBy(0.9f); }},
        {"toggle_playback", [](TimelineCanvasWidget &c) { c.togglePlaybackPublic(); }},
    };
    return commands;
}

void TimelineToolbarWidget::dispatch(WindowGState &windowState, TimelineCanvasWidget &canvasTarget,
                                     const ToolbarButton &button) {
    qWarning() << "TimelineToolbarWidget: script action not yet wired up:" << button.actionValue << "(id=" << button.id
               << ")";
}

void TimelineToolbarWidget::loadButtons(WindowGState &windowState, const QString &target,
                                        TimelineCanvasWidget &canvasTarget) {
    for (auto *btn : currentButtons) {
        layout->removeWidget(btn);
        btn->deleteLater();
    }
    currentButtons.clear();

    auto buttonsResult = esotereel::Toolbar::getButtons(windowState.network, target);
    if (buttonsResult.isError()) {
        qWarning() << "TimelineToolbarWidget: failed to load buttons for target" << target;
        return;
    }
    const auto buttons = buttonsResult.unwrapOrMove();

    int insertPos = 0;
    for (const auto &button : buttons) {
        auto *btn = new QToolButton(this);
        btn->setText(button.label);
        btn->setToolTip(button.tooltip.isEmpty() ? button.label : button.tooltip);
        // TODO: button.icon (パス文字列) からQIconを読み込んで setIcon する

        QObject::connect(btn, &QToolButton::clicked, this, [this, &windowState, &canvasTarget, button]() {
            this->dispatch(windowState, canvasTarget, button);
        });

        layout->insertWidget(insertPos++, btn);
        currentButtons.push_back(btn);
    }
}

} // namespace esotereel::window
