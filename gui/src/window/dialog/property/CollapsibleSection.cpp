#include "CollapsibleSection.h"

namespace esotereel::window::widget {

CollapsibleSection::CollapsibleSection(const QString &title, QWidget *parent) : QWidget(parent) {
    auto *outer = new QVBoxLayout(this);
    outer->setContentsMargins(0, 0, 0, 0);
    outer->setSpacing(4);

    headerButton = new QToolButton();
    headerButton->setText(title);
    headerButton->setCheckable(true);
    headerButton->setChecked(true);
    headerButton->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    headerButton->setArrowType(Qt::DownArrow);
    headerButton->setStyleSheet("QToolButton { border: none; font-weight: bold; }");
    outer->addWidget(headerButton);

    contentWidget = new QWidget();
    contentLayout_ = new QVBoxLayout(contentWidget);
    contentLayout_->setContentsMargins(12, 0, 0, 8);
    outer->addWidget(contentWidget);

    connect(headerButton, &QToolButton::toggled, this, [this](bool checked) {
        contentWidget->setVisible(checked);
        headerButton->setArrowType(checked ? Qt::DownArrow : Qt::RightArrow);
    });
}

} // namespace esotereel::window::widget
