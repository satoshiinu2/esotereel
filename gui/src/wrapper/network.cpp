#include "network.h"
#include "requests.h"

Requests ClientNetworkHandler::requests() const {
    return Requests(this);
}