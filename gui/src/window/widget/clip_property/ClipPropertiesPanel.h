#pragma once

#include <QTabWidget>
#include <QWidget>
#include <vector>

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

    // クリップの選択が変わるたびに呼ぶ。timelineId/clipIdsはsetValueの宛先として保持する。
    void setClips(const TimelineId timelineId, const std::vector<ClipId> &clipIds);

  private:
    void rebuild();
    QWidget *buildRow(const ClipPropertySchema &field);

    ClientState *state;
    CommandQueue &commandQueue_;

    TimelineId timelineId;
    std::vector<ClipId> clipIds;

    QTabWidget *topTabs;
};

} // namespace esotereel::window
