#include "Logger.h"
#include "esotereel_gui_helper.h"
#include "ffi/ClientNetworkHandler.h"
#include "ffi/InternalServer.h"
#include "ffi/Requests.h"
#include "network/boot.h"
#include "window/MainWindow.h"
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

using TimelineId = esotereel_gui_helper::TimelineId;

Q_LOGGING_CATEGORY(logRust, "lib")

void bootcore(QString corePath);
void onServerStart(bool ok);
void setCallBacks();

esotereel::window::MainWindow *window;
esotereel::ClientNetworkHandler network;
QString addr;

int main(int argc, char **argv) {
    QApplication app(argc, argv);

    setCallBacks();

    esotereel::window::MainWindow w(network);
    window = &w;
    w.show();

    addr = "0.0.0.0:12345";
    esotereel::InternalServer::start(addr, onServerStart);

    return app.exec();
}

void onServerStart(bool ok) {
    if (ok) {
        network.run(addr);
    }
}

void onConnectedCallBack() {
    // placeholder
    network.requests().newProject();
}

void setCallBacks() {
    esotereel_gui_helper::GuiCallbacks callbacks;

    callbacks.on_test = +[]() {};
    callbacks.mark_dirty_timeline = +[](TimelineId id) { window->markDirtyTimeline(id); };

    esotereel_gui_helper::init();
    esotereel_gui_helper::init_rust_logger(esotereel::qtLogCallback);
    esotereel_gui_helper::set_gui_callbacks(callbacks);
    esotereel_gui_helper::set_on_connected_callback(onConnectedCallBack);
}
