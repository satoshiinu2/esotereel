#include "esotereel_gui_helper.h"
#include <QWidget>
#include <QWindow>

namespace esotereel {
enum class LinuxDisplayType { UNKNOWN, XCB, WAYLAND };

LinuxDisplayType getLinuxDisplayType();

esotereel_gui_helper::NativeWindowHandle getNativeWindowHandle(QWindow *window);
esotereel_gui_helper::NativeWindowHandle getNativeWindowHandle(QWidget *widget);

void forceDesyncSubsurface(QWindow *window);
} // namespace esotereel