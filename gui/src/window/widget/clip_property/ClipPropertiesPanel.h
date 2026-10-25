#pragma once

#include <QTabWidget>
#include <QWidget>
#include <vector>
#include <map>
#include <set>

#include "ffi/Settings.h"
#include "ffi/project/ClipProperty.h"

namespace esotereel::window {

// クリップ選択時に出す、画像のようなタブ+折りたたみセクションのプロパティパネル。
// SettingsDialogと同じFieldControlFactoryでコントロールを生成するが、
// Rust側のグループ階層を、トップのタブと再帰的な折りたたみセクションとして表示する。
class ClipPropertiesPanel : public QWidget {
    Q_OBJECT

  public:
    ClipPropertiesPanel(ClientState *state, CommandQueue &commandQueue, QWidget *parent = nullptr);

    // クリップの選択が変わるたびに呼ぶ。timelineId/clipIdsはsetValueの宛先として保持する。
    void setClips(const TimelineId timelineId, const std::vector<ClipId> &clipIds);
    void refreshChangedFields();

  private:
    void rebuild();
    QWidget *buildRow(const ClipPropertySchema &field);

    ClientState *state;
    CommandQueue &commandQueue;

    TimelineId timelineId;
    std::vector<ClipId> clipIds;

    QTabWidget *topTabs;
    std::map<QString, QWidget*> fieldControls; // key -> control widget
    std::set<QString> pendingUpdates; // 更新待ちのフィールドキー
};

} // namespace esotereel::window
