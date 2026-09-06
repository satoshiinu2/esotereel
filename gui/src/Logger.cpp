#include "esotereel_gui_helper.h"
#include "ffi/StringView.h"
#include <QDebug>
#include <QString>

namespace esotereel {
void qtLogCallback(size_t level, esotereel_gui_helper::StringView target_view,
                   esotereel_gui_helper::StringView msg_view) {
    QString target = StringView::toQstring(target_view);
    QString message = StringView::toQstring(msg_view);

    // esotereel系以外のログ（wgpu, naga等）は、Warn(2)以下の深刻なもの以外無視する
    if (!target.startsWith("esotereel") && level > 2) {
        return;
    }

    QString levelName;
    switch (level) {
    case 1:
        levelName = "ERROR";
        break;
    case 2:
        levelName = "WARN ";
        break;
    case 3:
        levelName = "INFO ";
        break;
    case 4:
        levelName = "DEBUG";
        break;
    case 5:
        levelName = "TRACE";
        break;
    default:
        levelName = "LOG  ";
        break;
    }

    // ログレベルとターゲット名をメッセージに含める
    QString formatted = QString("%1 [%2] %3").arg(levelName, target, message);

    switch (level) {
    case 1:
        qCritical().noquote() << formatted;
        break;
    case 2:
        qWarning().noquote() << formatted;
        break;
    case 3:
        qInfo().noquote() << formatted;
        break;
    default:
        qDebug().noquote() << formatted;
        break;
    }
}

} // namespace esotereel