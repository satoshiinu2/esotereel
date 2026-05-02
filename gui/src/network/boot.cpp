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
            qDebug().noquote() << "[core stdout]" << coreProcess->readAllStandardOutput();
        });

    QAbstractSocket::connect(
        coreProcess, &QProcess::readyReadStandardError, [coreProcess]() {
            qDebug().noquote() << "[core stderr]" << coreProcess->readAllStandardError();
        });
    coreProcess->start();

    qDebug() << "Core process started (non-blocking). Client will attempt to connect.";
}
