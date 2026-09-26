#include "Logger.h"
#include "esotereel_gui_helper.h"
#include "ffi/ClientState.h"
#include "ffi/InternalServer.h"
#include "ffi/Requests.h"
#include "ffi/StringView.h"
#include "network/boot.h"
#include "window/MainWindow.h"
#include <QApplication>
#include <QDebug>
#include <QLoggingCategory>
#include <QMessageBox>
#include <QProcess>
#include <QRegularExpression>
#include <QTimer>
#include <QWidget>
#include <qcontainerfwd.h>
#include <qdebug.h>
#include <qglobal.h>
#include <stdexcept>

using TimelineId = esotereel_gui_helper::TimelineId;
using namespace esotereel;

Q_LOGGING_CATEGORY(logRust, "lib")

void bootcore(QString corePath);
void startInternalServer();
void onServerStart(bool ok, esotereel_gui_helper::FfiStringView addr_ffi);
void onConnectedCallBack();

window::MainWindow *mainWindow;
ClientState *state;
QString addr;

int main(int argc, char **argv) {
    QApplication app(argc, argv);

    esotereel_gui_helper::init_rust_logger(qtLogCallback);

    QString stdPluginDir = qEnvironmentVariable("ESOTEREEL_PLUGIN_DIR");
    QString workingDir = qEnvironmentVariable("ESOTEREEL_WORKING_DIR");

    esotereel_gui_helper::GuiCallbacks callbacks;

    callbacks.on_test = +[]() {};
    callbacks.mark_dirty_timeline = +[](TimelineId id) { mainWindow->markDirtyTimeline(id); };
    callbacks.on_connected = +[]() { onConnectedCallBack(); };

    ClientState n(callbacks, stdPluginDir, workingDir);
    state = &n;

    n.logDirectoriesInfo();
    n.bootstrap();

    window::MainWindow w(n);
    mainWindow = &w;
    w.show();

    startInternalServer();

    try {
        return app.exec();
    } catch (const std::runtime_error &e) {
        qCWarning(logRust) << e.what();
        QMessageBox::critical(nullptr, "Critical error", e.what());
        return 1;
    }
}

void startInternalServer() {
    addr = "0.0.0.0:12345";
    QString stdPluginDir = qEnvironmentVariable("ESOTEREEL_PLUGIN_DIR");
    QString workingDir = qEnvironmentVariable("ESOTEREEL_WORKING_DIR");

    InternalServer::start(*state, addr, onServerStart, stdPluginDir, workingDir);
}

void onServerStart(bool ok, esotereel_gui_helper::FfiStringView addr_ffi) {
    if (ok) {
        state->run(StringView::toQString(addr_ffi));
    }
}

void onConnectedCallBack() {
    // placeholder
    state->requests().newProject();
}
