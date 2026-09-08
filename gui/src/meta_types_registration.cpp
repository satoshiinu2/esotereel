#include "esotereel_gui_helper.h"
#include "ffi/project/Timeline.h"
#include "ffi/project/camera.h"
#include <QMetaType>
#include <QtCore/qcoreapplication.h>

Q_DECLARE_METATYPE(esotereel::Timeline)
Q_DECLARE_METATYPE(esotereel::CameraInfo)

static void registerAppMetaTypes() {
    qRegisterMetaType<esotereel::Timeline>("Timeline");
    qRegisterMetaType<esotereel::CameraInfo>("CameraInfo");
}
Q_COREAPP_STARTUP_FUNCTION(registerAppMetaTypes)