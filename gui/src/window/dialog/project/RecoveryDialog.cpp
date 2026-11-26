#include "RecoveryDialog.h"
#include "ffi/ProjectFiles.h"
#include "ffi/Requests.h"
#include "window/MainWindow.h"

#include <QDateTime>
#include <QHBoxLayout>
#include <QLabel>
#include <QVBoxLayout>

namespace esotereel::window::dialog {

RecoveryDialog::RecoveryDialog(WindowGState &windowState, QWidget *parent)
    : QDialog(parent), windowState(windowState) {
    setWindowTitle(tr("Autosave Recovery"));
    resize(560, 360);

    auto *mainLayout = new QVBoxLayout(this);
    mainLayout->addWidget(
        new QLabel(tr("Autosave generations (newest first). Restoring opens the project "
                      "from that generation.")));

    generationList = new QListWidget();
    mainLayout->addWidget(generationList);

    auto *buttonLayout = new QHBoxLayout();
    restoreButton = new QPushButton(tr("Restore Selected"));
    restoreButton->setEnabled(false);
    auto *newProjectButton = new QPushButton(tr("New Project"));
    auto *closeButton = new QPushButton(tr("Close"));
    buttonLayout->addWidget(restoreButton);
    buttonLayout->addWidget(newProjectButton);
    buttonLayout->addStretch();
    buttonLayout->addWidget(closeButton);
    mainLayout->addLayout(buttonLayout);

    connect(generationList, &QListWidget::itemSelectionChanged, this, [this]() {
        restoreButton->setEnabled(generationList->currentItem() != nullptr);
    });
    connect(restoreButton, &QPushButton::clicked, this, &RecoveryDialog::onRestoreClicked);
    connect(newProjectButton, &QPushButton::clicked, this, &RecoveryDialog::onNewProjectClicked);
    connect(closeButton, &QPushButton::clicked, this, &QDialog::reject);

    // 一覧はサーバーへ問い合わせて非同期に受け取る。
    windowState.state->requests().listAutosaveGenerations();
    pollTimer = new QTimer(this);
    connect(pollTimer, &QTimer::timeout, this, &RecoveryDialog::pollGenerations);
    pollTimer->start(200);
}

void RecoveryDialog::pollGenerations() {
    ++pollAttempts;
    auto result = ProjectFiles::takeGenerations(windowState.state);
    if (result.isOk()) {
        auto generations = result.unwrap();
        if (!generations.isEmpty() || pollAttempts > 25) {
            pollTimer->stop();
            for (const auto &generation : generations) {
                auto *item = new QListWidgetItem(generationList);
                auto created = QDateTime::fromMSecsSinceEpoch(
                    static_cast<qint64>(generation.createdAtMs));
                item->setText(tr("%1  —  project %2  —  %3 KB")
                                  .arg(created.toString("yyyy-MM-dd HH:mm:ss"))
                                  .arg(generation.projectId.left(8))
                                  .arg(generation.sizeBytes / 1024));
                item->setData(Qt::UserRole, generation.id);
            }
            if (generations.isEmpty()) {
                auto *item = new QListWidgetItem(tr("(no autosave generations)"), generationList);
                item->setFlags(Qt::NoItemFlags);
            }
        }
    } else if (pollAttempts > 25) {
        pollTimer->stop();
    }
}

void RecoveryDialog::onRestoreClicked() {
    auto *item = generationList->currentItem();
    if (!item) {
        return;
    }
    windowState.state->requests().restoreAutosaveGeneration(item->data(Qt::UserRole).toString());
    accept();
}

void RecoveryDialog::onNewProjectClicked() {
    windowState.state->requests().newProject();
    accept();
}

} // namespace esotereel::window::dialog
