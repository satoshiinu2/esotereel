#include "window/main.h"
#include "client.h"
#include "nomyoedit_gui_helper.h"
#include <QApplication>
#include <QDebug>
#include <QProcess>
#include <QWidget>
#include <qdebug.h>
#include <qglobal.h>

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

    return app.exec();
}


// placeholder
void bootcore(QString corePath) {
    QProcess *coreProcess = new QProcess();

    coreProcess->setProgram(corePath);
    QAbstractSocket::connect(
        coreProcess, &QProcess::readyReadStandardOutput, [coreProcess]() {
            qDebug() << "[core stdout]" << coreProcess->readAllStandardOutput();
        });

    QAbstractSocket::connect(
        coreProcess, &QProcess::readyReadStandardError, [coreProcess]() {
            qDebug() << "[core stderr]" << coreProcess->readAllStandardError();
        });
    coreProcess->start();

    if (!coreProcess->waitForStarted()) {
        qDebug() << "failed to start core";
    } else {
        qDebug() << "core started!";
    }


    client.connectToCore();
}


void on_send_cb(const uint8_t *ptr, size_t len) {
    QByteArray data(reinterpret_cast<const char *>(ptr), len);
    client.send(data);
}

void setCallBacks(){
    nomyoedit_gui_helper::GuiCallbacks callbacks;
    
    callbacks.on_test = +[]() {
    };
    callbacks.on_update_timeline = +[](size_t id) {
        window->onUpdateTimeline(id);
    };

    nomyoedit_gui_helper::init();
    nomyoedit_gui_helper::set_gui_callbacks(callbacks);
    nomyoedit_gui_helper::set_send_callback(on_send_cb);
}
