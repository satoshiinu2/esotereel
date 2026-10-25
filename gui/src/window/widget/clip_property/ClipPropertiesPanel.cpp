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
#include <QSpinBox>
#include <QSignalBlocker>
#include <functional>
#include <QVBoxLayout>

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
            return defaultValue;
        }
        Project project = std::move(projectResult.unwrap());
        Timeline timeline = project.timelineOf(timelineId);
        auto [clip, layerId] = timeline.findClipById(clipIds[0]);

        auto result = getValue(clip, key);
        if (result.isOk()) {
            auto opt = result.unwrap();
            if (opt.has_value() && opt->isStatic()) {
                return opt->asStatic();
            }
            // TODO(Keyframes): opt->isKeyframes() の場合は現在のプレイヘッド位置の値を評価して返す
        }
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

    qDebug() << "[ClipPropertiesPanel] rebuild:"
             << "timelineId =" << timelineId << "clipIds count =" << clipIds.size();

    qDebug() << "[ClipPropertiesPanel] fields =" << fields.size();

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
                layout->addWidget(buildRow(field));
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
    for (const auto &field : fields) {
        const QString root = categoryPath(field).value(0, "General");
        if (roots.contains(root))
            continue;
        roots.append(root);
        QVector<ClipPropertySchema> children;
        for (const auto &candidate : fields) {
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
        return;
    }
    Project project = std::move(projectResult.unwrap());
    Timeline timeline = project.timelineOf(timelineId);
    auto [clip, layerId] = timeline.findClipById(clipIds[0]);

    for (const QString &key : pendingUpdates) {
        auto result = getValue(clip, key);
        if (!result.isOk()) {
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
        if (auto *spinBox = qobject_cast<QSpinBox*>(control)) {
            auto intValue = value.asInt();
            if (intValue.has_value()) {
                spinBox->setValue(intValue.value());
            }
        } else if (auto *doubleSpinBox = qobject_cast<QDoubleSpinBox*>(control)) {
            auto floatValue = value.asFloat();
            if (floatValue.has_value()) {
                doubleSpinBox->setValue(floatValue.value());
            }
        } else if (auto *checkBox = qobject_cast<QCheckBox*>(control)) {
            auto boolValue = value.asBool();
            if (boolValue.has_value()) {
                checkBox->setChecked(boolValue.value());
            }
        } else if (auto *comboBox = qobject_cast<QComboBox*>(control)) {
            auto qstringValue = value.asQString();
            if (qstringValue.has_value()) {
                comboBox->setCurrentText(qstringValue.value());
            }
        } else if (auto *lineEdit = qobject_cast<QLineEdit*>(control)) {
            auto qstringValue = value.asQString();
            if (qstringValue.has_value()) {
                lineEdit->setText(qstringValue.value());
            }
        } else if (const auto values = value.asArray()) {
            const auto components = control->findChildren<QDoubleSpinBox *>(QString(), Qt::FindDirectChildrenOnly);
            for (qsizetype index = 0; index < components.size() && index < static_cast<qsizetype>(values->size()); ++index) {
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
