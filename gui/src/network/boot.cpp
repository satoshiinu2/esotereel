#include <QAbstractSocket>
#include <QApplication>
#include <QProcess>
#include <QString>
#include <qdebug.h>

void bootcore(QString corePath) {
    qDebug() << "booting core: " << corePath;

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
}
