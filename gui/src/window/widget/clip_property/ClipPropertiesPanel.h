#pragma once

#include <QTabWidget>
#include <QWidget>

#include "ffi/ClipProperty.h"
#include "ffi/Settings.h"

namespace esotereel::window {

// クリップ選択時に出す、画像のようなタブ+折りたたみセクションのプロパティパネル。
// SettingsDialogと同じFieldControlFactoryでコントロールを生成するが、
// レイアウトはリストではなく category を " > " で分割した階層(タブ > タブ > 折りたたみセクション)を組む。
class ClipPropertiesPanel : public QWidget {
    Q_OBJECT

  public:
    ClipPropertiesPanel(ClientState *state, CommandQueue &commandQueue, QWidget *parent = nullptr);

    // クリップの選択が変わるたびに呼ぶ。timelineId/clipIdはsetValueの宛先として保持する。
    void setClip(const TimelineId timelineId, const ClipId clipId);

  private:
    void rebuild();
    QWidget *buildRow(const ClipPropertySchema &field);

    ClientState *state;
    CommandQueue &commandQueue_;

    TimelineId timelineId;
    ClipId clipId{};

    QTabWidget *topTabs;
};

} // namespace esotereel::window
