#include "window/main.h"
#include "client.h"
#include "muscedit_lib.h"
#include <QApplication>
#include <QDebug>
#include <QProcess>
#include <QWidget>
#include <qdebug.h>
#include <qglobal.h>

void bootcore(QString corePath);

int main(int argc, char **argv) {
    QApplication app(argc, argv);

    MainWindow w;
    w.show();

    if (argc < 2) {
        qDebug() << "Usage: gui <code_path>";
    }
    QString corePath = argv[1];
    bootcore(corePath);

    return app.exec();
}
Client client;

void on_send_cb(const uint8_t *ptr, size_t len);

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

    muscedit_lib::CommandCallbacks callbacks;
    callbacks.on_test = +[]() {
        qDebug() << "recv test!";
    };

    muscedit_lib::set_command_callbacks(callbacks);
    muscedit_lib::set_send_callbacks(on_send_cb);

    client.connectToCore();
}

void on_send_cb(const uint8_t *ptr, size_t len) {
    QByteArray data(reinterpret_cast<const char *>(ptr), len);
    client.send(data);
}
