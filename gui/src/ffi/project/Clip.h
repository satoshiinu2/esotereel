#pragma once

#include "esotereel_gui_helper.h"
#include "ffi/ClipPropertyValue.h"
#include "ffi/CommandQueue.h"
#include "ffi/FieldValue.h"
#include "ffi/Result.h"
#include "ffi/StringView.h"
#include <QString>
#include <QStringList>
#include <QVector>
#include <variant>

namespace esotereel {
class ClientState;

using RawClip = esotereel_gui_helper::Clip;
using ClipId = esotereel_gui_helper::ClipId;
using TimelineId = esotereel_gui_helper::TimelineId;
using TimelineTick = esotereel_gui_helper::TimelineTick;
using CClipBindingValueTag = esotereel_gui_helper::CClipBindingValueTag;
using CClipBindingValue = esotereel_gui_helper::CClipBindingValue;

class Clip {
  public:
    const RawClip *ptr_clip;

    Clip(const RawClip *p) noexcept;

    static Clip Empty();

    bool isValid() const noexcept;

    operator const RawClip *() const noexcept {
        return ptr_clip;
    }

    ClipId id() const noexcept;
    TimelineTick position() const noexcept;
    TimelineTick duration() const noexcept;

    Result<ClipPropertyValue> getPropertyValue(const QString &key) const;
    Result<QVector<ClipPropertySchema>> getAllFields(ClientState *network) const;
    Result<QStringList> getCategories(ClientState *network) const;

    Result<void> setPropertyValue(CommandQueue &commandQueue, TimelineId timelineId, const QString &key,
                                  const ClipPropertyValue &value) const;

    void freeC(CClipBindingValue &value);
};

} // namespace esotereel
