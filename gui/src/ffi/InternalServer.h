#pragma once

#include <QString>

namespace esotereel::InternalServer {
bool start(QString addr, void (*OnConnectedFn)(bool), QString stdPluginDir = QString(), QString workingDir = QString());
} // namespace esotereel::InternalServer