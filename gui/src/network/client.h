#pragma once

#include <QObject>
#include <QTcpSocket>

class Client : public QObject {
    Q_OBJECT

  public:
    QTcpSocket socket;

    Client();
    void connectToCore();
    void send(const QByteArray &data);
};