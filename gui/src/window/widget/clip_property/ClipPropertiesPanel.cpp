#include "ClipPropertiesPanel.h"
#include "ffi/ClipProperty.h"
#include "ffi/project/Project.h"
#include "ffi/project/Timeline.h"
#include "window/dialog/property/CollapsibleSection.h"
#include "window/dialog/property/FieldControlFactory.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QMap>
#include <QVBoxLayout>

namespace esotereel::window {

ClipPropertiesPanel::ClipPropertiesPanel(ClientState *state, esotereel::CommandQueue &commandQueue, QWidget *parent)
    : QWidget(parent), state(state), commandQueue_(commandQueue) {
    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);

    topTabs = new QTabWidget();
    layout->addWidget(topTabs);
}

void ClipPropertiesPanel::setClip(const TimelineId timelineId, const ClipId clipId) {
    this->timelineId = timelineId;
    this->clipId = clipId;
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
    binding.getValue = [this, key]() -> FieldValue {
        auto projectResult = state->getProject();
        if (projectResult.isError()) {
            return FieldValue{};
        }
        Project project = projectResult.unwrapOrMove();
        Timeline timeline = project.timelineOf(timelineId);
        auto [clip, layerId] = timeline.findClipById(clipId);

        auto result = getValue(clip, key);
        if (result.isOk()) {
            auto opt = result.unwrap();
            if (opt.has_value() && opt->isStatic()) {
                return opt->asStatic();
            }
            // TODO(Keyframes): opt->isKeyframes() の場合は現在のプレイヘッド位置の値を評価して返す
        }
        return FieldValue{}; // フォールバック。field.defaultValueを使う形に調整してください
    };
    binding.setValue = [this, key](const FieldValue &value) {
        setValue(commandQueue_, timelineId, clipId, key, ClipPropertyValue::fromStatic(value));
    };

    QWidget *control = widget::FieldControlFactory::createControl(field, binding);
    rowLayout->addWidget(control);

    return row;
}

void ClipPropertiesPanel::rebuild() {
    topTabs->clear();

    auto projectResult = state->getProject();
    if (projectResult.isError()) {
        return;
    }
    Project project = projectResult.unwrapOrMove();
    Timeline timeline = project.timelineOf(timelineId);
    auto [clip, layerId] = timeline.findClipById(clipId);

    qDebug() << "[ClipPropertiesPanel] clip isValid:" << clip.isValid();

    auto fieldsResult = getAllFields(state, clip);
    if (!fieldsResult.isOk()) {
        qWarning() << "Failed to get clip fields:" << QString::fromStdString(fieldsResult.error());
        return;
    }
    const QVector<ClipPropertySchema> fields = fieldsResult.unwrap();

    qDebug() << "[ClipPropertiesPanel] rebuild:"
             << "timelineId =" << timelineId << "clipId =" << clipId;

    qDebug() << "[ClipPropertiesPanel] fields =" << fields.size();

    // category は "動画 > ベーシック > 位置とサイズ" のように " > " 区切りで
    // タブ > タブ > 折りたたみセクション の3階層に対応させる。
    // 第1階層(タブ) -> 第2階層(タブ) -> 第3階層(セクション名) -> フィールド一覧
    QMap<QString, QMap<QString, QMap<QString, QVector<ClipPropertySchema>>>> tree;
    for (const auto &field : fields) {
        const QStringList parts = field.category.split(" > ");
        const QString top = parts.value(0, "General");
        const QString sub = parts.value(1, "General");
        const QString section = parts.value(2, "General");
        tree[top][sub][section].append(field);

        qDebug() << "field:" << field.key << "category:" << field.category;
    }

    for (auto topIt = tree.constBegin(); topIt != tree.constEnd(); ++topIt) {
        qDebug() << "top:" << topIt.key();

        auto *subTabs = new QTabWidget();

        for (auto subIt = topIt.value().constBegin(); subIt != topIt.value().constEnd(); ++subIt) {
            auto *page = new QWidget();
            auto *pageLayout = new QVBoxLayout(page);
            qDebug() << "  sub:" << subIt.key();

            for (auto sectionIt = subIt.value().constBegin(); sectionIt != subIt.value().constEnd(); ++sectionIt) {
                auto *section = new widget::CollapsibleSection(sectionIt.key());
                for (const auto &field : sectionIt.value()) {
                    section->contentLayout()->addWidget(buildRow(field));
                }
                pageLayout->addWidget(section);
                qDebug() << "    section:" << sectionIt.key() << "fields:" << sectionIt.value().size();
            }
            pageLayout->addStretch();

            subTabs->addTab(page, subIt.key());
        }

        topTabs->addTab(subTabs, topIt.key());
    }
}

} // namespace esotereel::window
