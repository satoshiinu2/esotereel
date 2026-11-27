#pragma once

#include <QVector>
#include <QWidget>

#include "ffi/Assets.h"

class QPushButton;
class QTableWidget;

namespace esotereel::window {

// Assets パネルからタイムラインへのドラッグで使う MIME 型。
// ペイロードは CAssetRef の16バイトを文字列化せずそのまま載せる。
inline constexpr char kAssetRefMimeType[] = "application/x-esotereel-asset-ref";

// サーバーから同期されたアセット一覧を表示するドックパネル。
// 一覧の実データは guihlp 側の同期済みアセット表で、このパネルは
// reload() のたびに読み直して表示するだけ。変更（名前変更・削除・
// 再リンク）は要求として送り、結果は次の同期で反映される。
class AssetsPanel : public QWidget {
    Q_OBJECT

  public:
    explicit AssetsPanel(ClientState *state, QWidget *parent = nullptr);

    // 同期済みアセット表を読み直して表示を更新する。
    void reload();

  private:
    void updateButtonStates();
    const AssetEntry *selectedEntry() const;

    void onRenameClicked();
    void onRemoveClicked();
    void onRelinkClicked();

    ClientState *state;
    QVector<AssetEntry> entries;

    QTableWidget *table;
    QPushButton *renameButton;
    QPushButton *removeButton;
    QPushButton *relinkButton;
};

} // namespace esotereel::window
