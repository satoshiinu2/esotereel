#pragma once

#include "../../util.h"
#include "../../wrapper/wgpuutil.h"
#include "../main.h"
#include <QResizeEvent>
#include <QPlatformSurfaceEvent>
#include <QTimer>
#include <QWidget>
#include <QWindow>
#include <optional>

// 実際にVulkanでレンダリングする側。QWidgetの子ネイティブウィンドウ(subsurface)としてではなく、
// createWindowContainer経由で埋め込む独立したQWindowにすることで、
// Qt自身のバックストア管理から完全に外れたサーフェスを持たせる。
class WgpuRenderWindow : public QWindow {
    Q_OBJECT

  public:
    explicit WgpuRenderWindow(WindowGState *windowState, QWindow *parent = nullptr);

    WgpuRenderWindow(const WgpuRenderWindow &) = delete;
    WgpuRenderWindow &operator=(const WgpuRenderWindow &) = delete;

  protected:
    void exposeEvent(QExposeEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;
    bool event(QEvent *event) override;

  private:
    bool requestRender();
    void ensureInitialized();

    WindowGState *windowState;
    std::optional<WGpuUtil> wgpuutil;
    QTimer *renderTimer = nullptr;
};

// レイアウトに置けるようにするための薄いラッパー。
// 中身の実体はWgpuRenderWindow(QWindow)で、createWindowContainerで包んでいるだけ。
class WgpuCanvasWidget : public QWidget {
    Q_OBJECT

  public:
    explicit WgpuCanvasWidget(WindowGState *windowState);

    WgpuCanvasWidget(const WgpuCanvasWidget &) = delete;
    WgpuCanvasWidget(WgpuCanvasWidget &&) = delete;
    WgpuCanvasWidget &operator=(const WgpuCanvasWidget &) = delete;
    WgpuCanvasWidget &operator=(WgpuCanvasWidget &&) = delete;

  private:
    WgpuRenderWindow *renderWindow = nullptr;
    QWidget *container = nullptr;
};
