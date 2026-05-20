#pragma once

#include <cstdint>
class Clip;
class Layer;
class ClipsIterator;
class ClipsIterable;
class Timeline;
class LayersIterator;
class LayersIterable;
class Project;

struct SlotMapKey {
    uintptr_t index;
    uint32_t generation;
};