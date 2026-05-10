#pragma once

#include <QString>

namespace InternalServer {
bool start(QString addr, void (*OnConnectedFn)(bool));
} // namespace InternalServer