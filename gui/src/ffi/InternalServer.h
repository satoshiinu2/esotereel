#pragma once

#include <QString>

namespace esotereel::InternalServer {
bool start(QString addr, void (*OnConnectedFn)(bool));
} // namespace esotereel::InternalServer