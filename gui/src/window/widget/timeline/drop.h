#pragma once

#include <QDropEvent>
#include <QMimeData>
#include <QPoint>
#include <QStringList>

namespace esotereel::window {

class TimelineCanvasWidget;

/// MIME データからドロップされたローカルファイルパスを抽出する。
QStringList extractDropFilePaths(const QMimeData *mimeData);

/// タイムライン上のドロップ位置にビデオクリップを追加する。
/// canvas のプライベートメンバー（project, cachedRows, commandQueue 等）にアクセスするため
/// TimelineCanvasWidget の friend 関数として宣言する。
bool handleVideoFileDrop(TimelineCanvasWidget *canvas, const QDropEvent *event);

} // namespace esotereel::window
