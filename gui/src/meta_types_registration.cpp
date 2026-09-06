#include "esotereel_gui_helper.h"
#include "ffi/project/Timeline.h"
#include "ffi/project/camera.h"
#include <QMetaType>
#include <QtCore/qcoreapplication.h>

namespace esotereel {
using NativeWindowHandle = esotereel_gui_helper::NativeWindowHandle;
}

Q_DECLARE_METATYPE(esotereel::Timeline)
Q_DECLARE_METATYPE(esotereel::CameraInfo)
Q_DECLARE_METATYPE(esotereel::NativeWindowHandle)

static void registerAppMetaTypes() {
    qRegisterMetaType<esotereel::Timeline>("Timeline");
    qRegisterMetaType<esotereel::CameraInfo>("CameraInfo");
    qRegisterMetaType<esotereel::NativeWindowHandle>("NativeWindowHandle");
}
Q_COREAPP_STARTUP_FUNCTION(registerAppMetaTypes)