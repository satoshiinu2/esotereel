#include "window/main.h"
#include "esotereel_gui_helper.h"
#include "network/boot.h"
#include "network/client.h"
#include <QApplication>
#include <QDebug>
#include <QLoggingCategory>
#include <QProcess>
#include <QWidget>
#include <qdebug.h>
#include <qglobal.h>

Q_LOGGING_CATEGORY(logRust, "rust.core")

void bootcore(QString corePath);
void setCallBacks();

MainWindow *window;
Client client;

int main(int argc, char **argv) {
    QApplication app(argc, argv);

    setCallBacks();

    MainWindow w;
    window = &w;
    w.show();

    if (argc < 2) {
        qDebug() << "Usage: gui <code_path>";
    }
    QString corePath = argv[1];
    bootcore(corePath);
    client.connectToCore();

    return app.exec();
}

void on_send_cb(const uint8_t *ptr, size_t len) {
    QByteArray data(reinterpret_cast<const char *>(ptr), len);
    client.send(data);
}

void q_log_callback(size_t level, const uint8_t *ptr, size_t len) {
    QString message = QString::fromUtf8(reinterpret_cast<const char *>(ptr), static_cast<int>(len));

    switch (level) {
    case 1:
        qCritical(logRust).noquote() << message;
        break;
    case 2:
        qWarning(logRust).noquote() << message;
        break;
    case 3:
        qInfo(logRust).noquote() << message;
        break;
    default:
        qDebug(logRust).noquote() << message;
        break;
    }
}

void setCallBacks() {
    esotereel_gui_helper::_GuiCallbacks callbacks;

    callbacks.on_test = +[]() {
    };
    callbacks.on_update_timeline = +[](size_t id) {
        window->onUpdateTimeline(id);
    };

    esotereel_gui_helper::init();
    esotereel_gui_helper::init_rust_logger(q_log_callback);
    esotereel_gui_helper::set_gui_callbacks(callbacks);
    esotereel_gui_helper::set_send_callback(on_send_cb);
}
