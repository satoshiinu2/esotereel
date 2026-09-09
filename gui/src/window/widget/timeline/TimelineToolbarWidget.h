#pragma once

#include <QHBoxLayout>
#include <QToolButton>
#include <QWidget>
#include <functional>
#include <unordered_map>
#include <vector>

#include "ffi/Toolbar.h"

namespace esotereel::window {

class TimelineCanvasWidget;
struct WindowGState;

// タイムラインの外側・上部に置くツールバー。
// ボタンの種類・並び順・アクション定義はRust側(plugin::toolbar)から
// FFI経由で取得し、Qt側は「もらったものを並べて、押されたら振り分ける」だけ。
class TimelineToolbarWidget : public QWidget {
    Q_OBJECT

  public:
    explicit TimelineToolbarWidget(QWidget *parent = nullptr);

    // FFI経由でボタン一覧(レイアウト順)を取得し、実際にQToolButtonを並べる。
    // targetは"timeline"等、Rust側のToolbarButtonSpec::targetに対応する識別子。
    void loadButtons(WindowGState &windowState, const QString &target, TimelineCanvasWidget &canvasTarget);

  private:
    QHBoxLayout *layout;
    std::vector<QToolButton *> currentButtons;

    // Builtinコマンド名 -> ネイティブ実行内容。
    // Rust側はコマンド名を「データ」として持つだけで、実体はこちら側にある。
    static const std::unordered_map<QString, std::function<void(TimelineCanvasWidget &)>> &builtinCommands();

    void dispatch(WindowGState &windowState, TimelineCanvasWidget &canvasTarget, const class ToolbarButton &button);
};

} // namespace esotereel::window
