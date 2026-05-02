#pragma once

#include "esotereel_gui_helper.h"
#include "stringview.h"
#include <QWidget>
#include <qcontainerfwd.h>
#include <qguiapplication_platform.h>
#include <qlogging.h>
#include <qwindowdefs.h>
#include <sys/types.h>

using RawClientNetworkHandler = esotereel_gui_helper::_ClientNetworkHandler;
using WrapperErrorCode = esotereel_gui_helper::_WrapperErrorCode;

class ClientNetworkHandler {

  private:
    const RawClientNetworkHandler *raw_ptr;
    bool isWayland;

  public:
    ClientNetworkHandler() {
        auto res = esotereel_gui_helper::client_network_handler_new(&raw_ptr);
        if (res != WrapperErrorCode::Ok) {
            qCritical() << "Failed to create ClientNetworkHandler:" << (int)res;
            raw_ptr = nullptr;
        }
    }
    ~ClientNetworkHandler() {
        if (raw_ptr) {
            esotereel_gui_helper::client_network_handler_drop(raw_ptr);
            raw_ptr = nullptr;
        }
    }
    ClientNetworkHandler(const ClientNetworkHandler &) = delete;
    ClientNetworkHandler &operator=(const ClientNetworkHandler &) = delete;

    //  move
    ClientNetworkHandler(ClientNetworkHandler &&other) noexcept : raw_ptr(other.raw_ptr) {
        other.raw_ptr = nullptr;
    }

    // drop
    ClientNetworkHandler &operator=(ClientNetworkHandler &&other) noexcept {
        if (this != &other) {
            if (raw_ptr) {
                esotereel_gui_helper::client_network_handler_drop(raw_ptr);
            }
            raw_ptr = other.raw_ptr;
            other.raw_ptr = nullptr;
        }
        return *this;
    }

    bool isValid() const { return raw_ptr != nullptr; }
    bool run(QString addr) {
        if (!isValid()) {
            return false;
        }

        QByteArray addrUtf8 = addr.toUtf8();
        auto addrView = StringView::fromQUtf8String(addrUtf8);
        
        auto res = esotereel_gui_helper::client_network_handler_run(raw_ptr, addrView);
        if (res != WrapperErrorCode::Ok) {
            qWarning() << "Failed to start network worker:" << (int)res;
        }
        return res == WrapperErrorCode::Ok;
    }
};