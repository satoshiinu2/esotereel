#include "Toolbar.h"
#include "ClientNetworkHandler.h"
#include "StringView.h"
#include "WrapperResult.h"

#include "esotereel_gui_helper.h"
#include "ffi/ClientNetworkHandler.h"

namespace esotereel {

Result<QVector<ToolbarButton>> Toolbar::getButtons(ClientNetworkHandler *network, const QString &target) {
    if (!network) {
        return Result<QVector<ToolbarButton>>::error("Network handler is null");
    }

    QVector<ToolbarButton> buttons;

    QByteArray targetUtf8 = target.toUtf8();
    RawStringView targetView = StringView::fromQUtf8String(targetUtf8);

    int32_t count = esotereel_gui_helper::toolbar_get_buttons_count(*network, targetView);
    if (count < 0) {
        return Result<QVector<ToolbarButton>>::error("Failed to get toolbar buttons count");
    }
    if (count == 0) {
        return Result<QVector<ToolbarButton>>::ok(buttons);
    }

    QVector<esotereel_gui_helper::FfiToolbarButton> ffiButtons(count);

    WrapperErrorCode result = esotereel_gui_helper::toolbar_get_buttons(*network, targetView, ffiButtons.data(), count);
    if (result != WrapperErrorCode::Ok) {
        return wrapperResultToResult<QVector<ToolbarButton>>(result, buttons);
    }

    for (int i = 0; i < count; ++i) {
        buttons.append(ToolbarButton(ffiButtons[i]));
    }

    return Result<QVector<ToolbarButton>>::ok(buttons);
}

Result<void> Toolbar::setLayout(ClientNetworkHandler *network, const QString &target, const QStringList &orderedIds) {
    if (!network) {
        return Result<void>::error("Network handler is null");
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

    WrapperErrorCode result = esotereel_gui_helper::toolbar_set_layout(*network, targetView, idsView);
    return wrapperResultToResultVoid(result);
}

Result<void> ToolbarButton::handleAction(ClientNetworkHandler *network) {
    QByteArray idUtf8 = this->id.toUtf8();
    RawStringView idView = StringView::fromQUtf8String(idUtf8);

    WrapperErrorCode result = esotereel_gui_helper::toolbar_handle_action(*network, idView);

    return wrapperResultToResultVoid(result);
}
} // namespace esotereel