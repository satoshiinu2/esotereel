#pragma once

#include <QString>
#include <cstddef>
#include <cstdint>
#include <vector>

class ClientNetworkHandler;
namespace esotereel_gui_helper {
struct _ClientNetworkHandler;
}

class Requests {
    const esotereel_gui_helper::_ClientNetworkHandler *raw_ptr;

  public:
    Requests(const ClientNetworkHandler *network);
    void newProject();
    void moveClips(uint64_t timelineIdx, const std::vector<uint64_t> &clipIds, int64_t posMoved, int64_t durationMoved,
                   int64_t layerMoved) noexcept;
    void addClipAt(uint64_t timelineIdx, int64_t position, size_t layerIdx) noexcept;
    void loadStream(QString path) noexcept;
};