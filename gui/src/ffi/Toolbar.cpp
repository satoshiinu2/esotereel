#include "Toolbar.h"
#include "ClientState.h"
#include "StringView.h"
#include "WrapperResult.h"
#include "Array.h"

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

    // ["id1","id2",...] 形式のTOML配列文字列にする(id自体に " は含まれない前提)
    QStringList quoted;
    quoted.reserve(orderedIds.size());
    for (const QString &id : orderedIds) {
        quoted.append(QStringLiteral("\"%1\"").arg(id));
    }
    QString idsArray = QStringLiteral("[%1]").arg(quoted.join(","));

    QByteArray targetUtf8 = target.toUtf8();
    QByteArray idsUtf8 = idsArray.toUtf8();
    RawStringView targetView = StringView::fromQUtf8String(targetUtf8);
    RawStringView idsView = StringView::fromQUtf8String(idsUtf8);

    auto result = esotereel_gui_helper::toolbar_set_layout(*state, targetView, idsView);

    if (!result.is_ok) {
        return Result<void>::err(OwnedString::intoStdString(result.err));
    }

    return Result<void>::ok();
}

Result<void> ToolbarButton::handleAction(ClientState *state) {
    QByteArray idUtf8 = this->id.toUtf8();
    RawStringView idView = StringView::fromQUtf8String(idUtf8);

    auto result = esotereel_gui_helper::toolbar_handle_action(*state, idView);

    if (!result.is_ok) {
        return Result<void>::err(OwnedString::intoStdString(result.err));
    }

    return Result<void>::ok();
}
} // namespace esotereel