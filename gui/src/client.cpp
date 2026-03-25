#include "client.h"
#include "muscedit_lib.h"
#include <QDebug>
#include <sys/types.h>

Client::Client() {
    connect(&socket, &QTcpSocket::readyRead, this, [&]() {
        QByteArray data = socket.readAll();
        const u_int8_t *ptr = reinterpret_cast<const uint8_t *>(data.constData());
        muscedit_lib::on_responce_recveve(ptr, data.length());
        // qDebug() << "recv:" << data;
    });
    connect(&socket, &QTcpSocket::connected, this, []() {
        muscedit_lib::cmd_new_project();
    });
}
void Client::connectToCore() {
    socket.connectToHost("127.0.0.1", 12345);
}

void Client::send(const QByteArray &data) { socket.write(data); }
