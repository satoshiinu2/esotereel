#pragma once

#include <QString>
#include <QVector>
#include <QWidget>
#include <array>
#include <cstdint>
#include <functional>
#include <optional>

#include "ffi/Assets.h"
#include "ffi/Settings.h"
#include "ffi/project/ClipProperty.h"

class QLabel;

namespace esotereel::window::widget {

// 値の読み書きを抽象化するバインディング。
// Settingsは Settings::getValue/setValue に、Clipプロパティは
// ClipProperty::getValue/setValue (+ ClipPropertyValue <-> FieldValue の変換) に
// それぞれ束縛して同じFactoryへ渡す。Factory自体はどこに値が保存されているか知らない。
struct FieldBinding {
    std::function<esotereel::FieldValue()> getValue;
    std::function<void(const esotereel::FieldValue &)> setValue;
    // AssetRef 型フィールド用（任意）。同期済みアセット一覧の取得と、
    // ファイルからの新規登録。未設定のバインディング（設定ダイアログ等）では
    // ピッカーは空の一覧になり、登録ボタンは出ない。
    std::function<QVector<esotereel::AssetEntry>()> listAssets;
    std::function<std::optional<std::array<uint8_t, 16>>(const QString &path)> registerAssetFile;
};

// AssetRef 型フィールドのエディタ。現在の参照先（名前と状態）を表示し、
// ボタンでピッカーを開く。値の再読み込みは refreshFromBinding()。
class AssetRefEditorWidget : public QWidget {
    Q_OBJECT
  public:
    AssetRefEditorWidget(const FieldBinding &binding, QWidget *parent = nullptr);

    void refreshFromBinding();

  private:
    void openPicker();

    FieldBinding binding_;
    QLabel *label_ = nullptr;
};

// SettingsDialog::createControlForField から切り出した「1フィールド分のコントロールを作る」部分。
// ラベルは含まない(呼び出し側がレイアウトに応じて自由に配置する)。
class FieldControlFactory {
  public:
    static QWidget *createControl(const SettingsField &field, const FieldBinding &binding);
    static QWidget *createControl(const ClipPropertySchema &field, const FieldBinding &binding);

  private:
    // Enumの選択肢はまだschemaに含まれていないため、暫定的にキー名から推測する。
    // schema側にoptionsが追加され次第、この関数は不要になる。
    static QStringList guessEnumOptions(const QString &key, const QString &currentValue);
};

} // namespace esotereel::window::widget
