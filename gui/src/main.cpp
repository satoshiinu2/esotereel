#include "window/main.h"
#include "log.h"
#include "network/boot.h"
#include "wrapper/internalserver.h"
#include "wrapper/network.h"
#include "wrapper/requests.h"
#include "wrapper/stringview.h"
#include <QApplication>
#include <QDebug>
#include <QLoggingCategory>
#include <QProcess>
#include <QRegularExpression>
#include <QTimer>
#include <QWidget>
#include <qcontainerfwd.h>
#include <qdebug.h>
#include <qglobal.h>

Q_LOGGING_CATEGORY(logRust, "lib")

void bootcore(QString corePath);
void setCallBacks();

MainWindow *window;
ClientNetworkHandler network;

int main(int argc, char **argv) {
    QApplication app(argc, argv);

    setCallBacks();

    MainWindow w;
    window = &w;
    w.show();

    QString addr = "0.0.0.0:12345";
    InternalServer::internalServerStart(addr);
    network.run(addr);

    return app.exec();
}

void onConnectedCallBack() {
    // placeholder
    Requests::newProject();
}

void setCallBacks() {
    esotereel_gui_helper::_GuiCallbacks callbacks;

    callbacks.on_test = +[]() {
    };
    callbacks.on_update_timeline = +[](size_t id) {
        window->onUpdateTimeline(id);
    };
    callbacks.on_stream_frame = +[](uint32_t resource_id, uint32_t width, uint32_t height, const uint8_t *data) {
    };

    esotereel_gui_helper::init();
    esotereel_gui_helper::init_rust_logger(q_log_callback);
    esotereel_gui_helper::set_gui_callbacks(callbacks);
    esotereel_gui_helper::set_on_connected_callback(onConnectedCallBack);
}
