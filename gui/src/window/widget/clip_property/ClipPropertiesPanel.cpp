#include "ClipPropertiesPanel.h"
#include "ffi/project/ClipProperty.h"
#include "ffi/project/Project.h"
#include "ffi/project/Timeline.h"
#include "window/dialog/property/CollapsibleSection.h"
#include "window/dialog/property/FieldControlFactory.h"

#include <QCheckBox>
#include <QComboBox>
#include <QDoubleSpinBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QMap>
#include <QSignalBlocker>
#include <QSpinBox>
#include <QVBoxLayout>
#include <functional>

namespace esotereel::window {

ClipPropertiesPanel::ClipPropertiesPanel(ClientState *state, esotereel::CommandQueue &commandQueue, QWidget *parent)
    : QWidget(parent), state(state), commandQueue(commandQueue) {
    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);

    topTabs = new QTabWidget();
    layout->addWidget(topTabs);
}

void ClipPropertiesPanel::setClips(const TimelineId timelineId, const std::vector<ClipId> &clipIds) {
    this->timelineId = timelineId;
    this->clipIds = clipIds;
    rebuild();
}

// フィールド1件分の行: ラベル(左) + コントロール(右)。
// 画像の「位置 X / 位置 Y」のようにフィールドが分かれている場合は、
// 今はそれぞれ独立した行として並ぶ(ペアで1行にまとめるのは将来の改善点)。
QWidget *ClipPropertiesPanel::buildRow(const ClipPropertySchema &field) {
    auto *row = new QWidget();
    auto *rowLayout = new QHBoxLayout(row);
    rowLayout->setContentsMargins(0, 0, 0, 0);

    rowLayout->addWidget(new QLabel(field.label));
    rowLayout->addStretch();

    const QString key = field.key;

    widget::FieldBinding binding;
    binding.getValue = [this, key, defaultValue = field.defaultValue]() -> FieldValue {
        if (clipIds.empty()) {
            return defaultValue;
        }
        auto projectResult = state->getProject();
        if (projectResult.isError()) {
            auto err = projectResult.getError();
            qWarning() << "ClipPropertiesPanel: getProject failed in getValue:" << QString::fromStdString(*err);
            return defaultValue;
        }
        Project project = std::move(projectResult.unwrap());
        Timeline timeline = project.timelineOf(timelineId);
        auto [clip, layerId] = timeline.findClipById(clipIds[0]);

        auto result = getValue(clip, key);
        if (result.isError()) {
            auto err = result.getError();
            qWarning() << "ClipPropertiesPanel: getValue failed:" << QString::fromStdString(*err);
            return defaultValue;
        }
        auto opt = result.unwrap();
            if (opt.has_value() && opt->isStatic()) {
                return opt->asStatic();
            }
            // TODO(Keyframes): opt->isKeyframes() の場合は現在のプレイヘッド位置の値を評価して返す
        return defaultValue;
    };
    binding.setValue = [this, key](const FieldValue &value) {
        for (const auto &clipId : clipIds) {
            setValue(commandQueue, timelineId, clipId, key, ClipPropertyValue::fromStatic(value));
        }
        commandQueue.sendAll();
        pendingUpdates.insert(key);
    };

    QWidget *control = widget::FieldControlFactory::createControl(field, binding);
    rowLayout->addWidget(control);

    // コントロールを登録
    fieldControls[key] = control;

    return row;
}

void ClipPropertiesPanel::rebuild() {
    topTabs->clear();
    fieldControls.clear();
    pendingUpdates.clear();

    if (clipIds.empty()) {
        return;
    }

    auto projectResult = state->getProject();
    if (projectResult.isError()) {
        auto err = projectResult.getError();
        qWarning() << "ClipPropertiesPanel: getProject failed in rebuild:" << QString::fromStdString(*err);
        return;
    }
    Project project = std::move(projectResult.unwrap());
    Timeline timeline = project.timelineOf(timelineId);

    std::vector<Clip> clips;
    for (const auto &clipId : clipIds) {
        auto [clip, layerId] = timeline.findClipById(clipId);
        clips.push_back(std::move(clip));
    }

    auto fieldsResult = getCommonFields(state, clips);
    if (!fieldsResult.isOk()) {
        auto err = fieldsResult.getError();
        assert(err.has_value());
        qWarning() << "Failed to get common clip fields:" << QString::fromStdString(*err);
        return;
    }
    const QVector<ClipPropertySchema> fields = fieldsResult.unwrap();

    // Map型もFieldControlFactoryで簡易表示（読み取り専用）に対応済みのため、
    // フィルタリングせずすべてのフィールドを編集パネルに表示する。
    QVector<ClipPropertySchema> editableFields = fields;

    qDebug() << "[ClipPropertiesPanel] rebuild:"
             << "timelineId =" << timelineId << "clipIds count =" << clipIds.size();

    qDebug() << "[ClipPropertiesPanel] fields =" << editableFields.size();

    // Rust projects typed groups into leaf fields with an ordered category path.
    // Keep that order and support arbitrary nesting instead of a fixed tab depth.
    const auto categoryPath = [](const ClipPropertySchema &field) {
        return field.category.split(" > ", Qt::SkipEmptyParts);
    };
    std::function<void(QVBoxLayout *, const QVector<ClipPropertySchema> &, qsizetype)> appendFields;
    appendFields = [&](QVBoxLayout *layout, const QVector<ClipPropertySchema> &items, qsizetype depth) {
        QStringList groups;
        for (const auto &field : items) {
            const auto path = categoryPath(field);
            if (path.size() <= depth) {
                if (field.kindType == esotereel_gui_helper::SettingsFieldType::Map) {
                    // Map型はツリー表示：見出しをコラプシブルセクションにし、
                    // メンバーを編集行として配置する（Transform > position/rotation/scale など）。
                    auto *section = new widget::CollapsibleSection(field.label);
                    if (auto mapValue = field.defaultValue.asMap()) {
                        for (const auto &[memberKey, memberValue] : *mapValue) {
                            ClipPropertySchema memberField;
                            memberField.key = field.key + "/" + QString::fromStdString(memberKey);
                            memberField.label = QString::fromStdString(memberKey);
                            memberField.category = field.category;

                            if (std::holds_alternative<FieldValue::Array>(memberValue.variant())) {
                                memberField.kindType = esotereel_gui_helper::SettingsFieldType::Array;
                            } else if (std::holds_alternative<double>(memberValue.variant())) {
                                memberField.kindType = esotereel_gui_helper::SettingsFieldType::Float;
                            } else if (std::holds_alternative<int64_t>(memberValue.variant())) {
                                memberField.kindType = esotereel_gui_helper::SettingsFieldType::Int;
                            } else if (std::holds_alternative<bool>(memberValue.variant())) {
                                memberField.kindType = esotereel_gui_helper::SettingsFieldType::Bool;
                            } else if (std::holds_alternative<RgbaColor>(memberValue.variant())) {
                                memberField.kindType = esotereel_gui_helper::SettingsFieldType::Color;
                            } else {
                                memberField.kindType = esotereel_gui_helper::SettingsFieldType::String;
                            }
                            memberField.defaultValue = memberValue;

                            section->contentLayout()->addWidget(buildRow(memberField));
                        }
                    }
                    layout->addWidget(section);
                } else {
                    layout->addWidget(buildRow(field));
                }
                continue;
            }
            const QString name = path[depth];
            if (groups.contains(name))
                continue;
            groups.append(name);
            QVector<ClipPropertySchema> children;
            for (const auto &candidate : items) {
                if (categoryPath(candidate).value(depth) == name)
                    children.append(candidate);
            }
            auto *section = new widget::CollapsibleSection(name);
            appendFields(section->contentLayout(), children, depth + 1);
            layout->addWidget(section);
        }
    };
    QStringList roots;
    for (const auto &field : editableFields) {
        const QString root = categoryPath(field).value(0, "General");
        if (roots.contains(root))
            continue;
        roots.append(root);
        QVector<ClipPropertySchema> children;
        for (const auto &candidate : editableFields) {
            if (categoryPath(candidate).value(0, "General") == root)
                children.append(candidate);
        }
        auto *page = new QWidget();
        auto *layout = new QVBoxLayout(page);
        appendFields(layout, children, 1);
        layout->addStretch();
        topTabs->addTab(page, root);
    }
}

void ClipPropertiesPanel::refreshChangedFields() {
    if (clipIds.empty() || pendingUpdates.empty()) {
        return;
    }

    auto projectResult = state->getProject();
    if (projectResult.isError()) {
        auto err = projectResult.getError();
        qWarning() << "ClipPropertiesPanel: getProject failed in refreshChangedFields:" << QString::fromStdString(*err);
        return;
    }
    Project project = std::move(projectResult.unwrap());
    Timeline timeline = project.timelineOf(timelineId);
    auto [clip, layerId] = timeline.findClipById(clipIds[0]);

    for (const QString &key : pendingUpdates) {
        auto result = getValue(clip, key);
        if (result.isError()) {
            auto err = result.getError();
            qWarning() << "ClipPropertiesPanel: getValue failed in refreshChangedFields:" << QString::fromStdString(*err);
            continue;
        }
        auto opt = result.unwrap();
        if (!opt.has_value() || !opt->isStatic()) {
            continue;
        }

        FieldValue value = opt->asStatic();
        auto it = fieldControls.find(key);
        if (it == fieldControls.end()) {
            continue;
        }

        QWidget *control = it->second;
        const QSignalBlocker blocker(control);

        // コントロールのタイプに応じて値を設定
        if (auto *spinBox = qobject_cast<QSpinBox *>(control)) {
            auto intValue = value.asInt();
            if (intValue.has_value()) {
                spinBox->setValue(intValue.value());
            }
        } else if (auto *doubleSpinBox = qobject_cast<QDoubleSpinBox *>(control)) {
            auto floatValue = value.asFloat();
            if (floatValue.has_value()) {
                doubleSpinBox->setValue(floatValue.value());
            }
        } else if (auto *checkBox = qobject_cast<QCheckBox *>(control)) {
            auto boolValue = value.asBool();
            if (boolValue.has_value()) {
                checkBox->setChecked(boolValue.value());
            }
        } else if (auto *comboBox = qobject_cast<QComboBox *>(control)) {
            auto qstringValue = value.asQString();
            if (qstringValue.has_value()) {
                comboBox->setCurrentText(qstringValue.value());
            }
        } else if (auto *lineEdit = qobject_cast<QLineEdit *>(control)) {
            auto qstringValue = value.asQString();
            if (qstringValue.has_value()) {
                lineEdit->setText(qstringValue.value());
            }
        } else if (const auto values = value.asArray()) {
            const auto components = control->findChildren<QDoubleSpinBox *>(QString(), Qt::FindDirectChildrenOnly);
            for (qsizetype index = 0; index < components.size() && index < static_cast<qsizetype>(values->size());
                 ++index) {
                const QSignalBlocker componentBlocker(components[index]);
                const auto &component = (*values)[index];
                if (auto number = component.asFloat())
                    components[index]->setValue(*number);
                else if (auto number = component.asInt())
                    components[index]->setValue(static_cast<double>(*number));
            }
        }
    }

    pendingUpdates.clear();
}

} // namespace esotereel::window
