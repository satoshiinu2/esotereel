#include "esotereel_gui_helper.h"
#include "wrapper/project/camera.h"
#include "wrapper/project/timeline.h"
#include <QMetaType>
#include <QtCore/qcoreapplication.h>

using NativeWindowHandle = esotereel_gui_helper::NativeWindowHandle;

Q_DECLARE_METATYPE(Timeline)
Q_DECLARE_METATYPE(CameraInfo)
Q_DECLARE_METATYPE(NativeWindowHandle)

static void registerAppMetaTypes() {
    qRegisterMetaType<Timeline>("Timeline");
    qRegisterMetaType<CameraInfo>("CameraInfo");
    qRegisterMetaType<NativeWindowHandle>("NativeWindowHandle");
}
Q_COREAPP_STARTUP_FUNCTION(registerAppMetaTypes)