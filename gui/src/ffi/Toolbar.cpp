#include "Toolbar.h"
#include "Array.h"
#include "ClientState.h"
#include "StringView.h"
#include "WrapperResult.h"

#include "esotereel_gui_helper.h"
#include "ffi/ClientState.h"

namespace esotereel {

Result<QVector<ToolbarButton>> Toolbar::getButtons(ClientState *state, const QString &target) {
    if (!state) {
        return Result<QVector<ToolbarButton>>::err("State is null");
    }

    QByteArray targetUtf8 = target.toUtf8();
    RawStringView targetView = StringView::fromQUtf8String(targetUtf8);

    auto result = esotereel_gui_helper::toolbar_get_buttons(*state, targetView);

    if (!result.is_ok) {
        return Result<QVector<ToolbarButton>>::err(OwnedString::intoStdString(result.value.err));
    }

    auto &array = result.value.ok;

    QVector<ToolbarButton> buttons;
    buttons.reserve(static_cast<int>(array.len));

    for (std::size_t i = 0; i < array.len; ++i) {
        buttons.append(ToolbarButton(array.ptr[i]));
    }

    // Free the array
    array.free_fn(&array);

    return Result<QVector<ToolbarButton>>::ok(buttons);
}

Result<void> Toolbar::setLayout(ClientState *state, const QString &target, const QStringList &orderedIds) {
    if (!state) {
        return Result<void>::err("State is null");
    }

    QByteArray targetUtf8 = target.toUtf8();
    RawStringView targetView = StringView::fromQUtf8String(targetUtf8);

    // FFI呼び出しが終わるまでUTF-8文字列の領域を維持する。
    std::vector<QByteArray> idsUtf8;
    idsUtf8.reserve(static_cast<std::size_t>(orderedIds.size()));
    for (const QString &id : orderedIds) {
        idsUtf8.push_back(id.toUtf8());
    }

    std::vector<RawStringView> idViews;
    idViews.reserve(idsUtf8.size());
    for (const QByteArray &id : idsUtf8) {
        idViews.push_back(StringView::fromQUtf8String(id));
    }

    auto idsArray = Array::fromVector(std::move(idViews));
    ArrayFreeGuard<RawStringView> idsGuard(idsArray);

    auto result = esotereel_gui_helper::toolbar_set_layout(*state, targetView, idsArray);

    if (!result.is_ok) {
        return Result<void>::err(OwnedString::intoStdString(result.err));
    }

    return Result<void>::ok();
}

Result<void> ToolbarButton::handleAction(ClientState *state, TimelineId timelineId) {
    QByteArray idUtf8 = this->id.toUtf8();
    RawStringView idView = StringView::fromQUtf8String(idUtf8);

    auto result = esotereel_gui_helper::toolbar_handle_action(*state, timelineId, idView);

    if (!result.is_ok) {
        return Result<void>::err(OwnedString::intoStdString(result.err));
    }

    return Result<void>::ok();
}
} // namespace esotereel