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
void onServerStart(bool ok);
void setCallBacks();

MainWindow *window;
ClientNetworkHandler network;
QString addr;

int main(int argc, char **argv) {
    QApplication app(argc, argv);

    setCallBacks();

    MainWindow w(network);
    window = &w;
    w.show();

    addr = "0.0.0.0:12345";
    InternalServer::start(addr, onServerStart);

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
    esotereel_gui_helper::_GuiCallbacks callbacks;

    callbacks.on_test = +[]() {};
    callbacks.redraw_timeline = +[](size_t id) { window->redrawTimeline(id); };

    esotereel_gui_helper::init();
    esotereel_gui_helper::init_rust_logger(q_log_callback);
    esotereel_gui_helper::set_gui_callbacks(callbacks);
    esotereel_gui_helper::set_on_connected_callback(onConnectedCallBack);
}
