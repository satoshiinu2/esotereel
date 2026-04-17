#include "client.h"
#include "../wrapper/requests.h"
#include "esotereel_gui_helper.h"
#include <QDebug>
#include <sys/types.h>

Client::Client() {
    connect(&socket, &QTcpSocket::readyRead, this, [&]() {
        QByteArray data = socket.readAll();
        const uint8_t *ptr = reinterpret_cast<const uint8_t *>(data.constData());
        esotereel_gui_helper::parse_responce(ptr, data.length());
        // qDebug() << "recv:" << data;
    });
    connect(&socket, &QTcpSocket::connected, this, []() {
        Requests::newProject();
    });
}
void Client::connectToCore() {
    socket.connectToHost("127.0.0.1", 12345);
}

void Client::send(const QByteArray &data) { socket.write(data); }
