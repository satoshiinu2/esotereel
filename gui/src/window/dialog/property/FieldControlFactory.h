#pragma once

#include <QString>
#include <QWidget>
#include <functional>

#include "ffi/ClipProperty.h"
#include "ffi/Settings.h"

namespace esotereel::window::widget {

// 値の読み書きを抽象化するバインディング。
// Settingsは Settings::getValue/setValue に、Clipプロパティは
// ClipProperty::getValue/setValue (+ ClipPropertyValue <-> FieldValue の変換) に
// それぞれ束縛して同じFactoryへ渡す。Factory自体はどこに値が保存されているか知らない。
struct FieldBinding {
    std::function<esotereel::FieldValue()> getValue;
    std::function<void(const esotereel::FieldValue &)> setValue;
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
