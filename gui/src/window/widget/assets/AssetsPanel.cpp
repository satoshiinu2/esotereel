#include "AssetsPanel.h"
#include "ffi/ClientState.h"
#include "ffi/Requests.h"
#include "window/widget/timeline/drop.h"

#include <QDragEnterEvent>
#include <QDragLeaveEvent>
#include <QDragMoveEvent>
#include <QDropEvent>
#include <QFileDialog>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QInputDialog>
#include <QLabel>
#include <QMessageBox>
#include <QMimeData>
#include <QPalette>
#include <QPushButton>
#include <QTableWidget>
#include <QVBoxLayout>
#include <array>
#include <functional>
#include <optional>

namespace esotereel::window {
namespace {

// ドラッグ専用の表。行のドラッグでカスタム MIME 型に CAssetRef の
// 16バイトを載せる。assetRefForRow が空を返す行（Missing 等）は
// mimeData が nullptr を返し、ドラッグが始まらない。
class AssetTableWidget : public QTableWidget {
  public:
    using QTableWidget::QTableWidget;

    // 行 → ドラッグするアセット参照。ドラッグ不可の行は空を返す。
    std::function<std::optional<std::array<uint8_t, 16>>(int row)> assetRefForRow;

    // ファイルドロップの処理はパネル側が持ち、表はイベントをそのまま渡す。
    std::function<void(QDragEnterEvent *)> dragEnterHandler;
    std::function<void(QDragMoveEvent *)> dragMoveHandler;
    std::function<void(QDragLeaveEvent *)> dragLeaveHandler;
    std::function<void(QDropEvent *)> dropHandler;

    void dragEnterEvent(QDragEnterEvent *event) override {
        if (dragEnterHandler) {
            dragEnterHandler(event);
        } else {
            QTableWidget::dragEnterEvent(event);
        }
    }

    void dragMoveEvent(QDragMoveEvent *event) override {
        if (dragMoveHandler) {
            dragMoveHandler(event);
        } else {
            QTableWidget::dragMoveEvent(event);
        }
    }

    void dragLeaveEvent(QDragLeaveEvent *event) override {
        if (dragLeaveHandler) {
            dragLeaveHandler(event);
        } else {
            QTableWidget::dragLeaveEvent(event);
        }
    }

    void dropEvent(QDropEvent *event) override {
        if (dropHandler) {
            dropHandler(event);
        } else {
            QTableWidget::dropEvent(event);
        }
    }

    QStringList mimeTypes() const override { return {QString::fromLatin1(kAssetRefMimeType)}; }

    QMimeData *mimeData(const QList<QTableWidgetItem *> &items) const override {
        if (items.isEmpty() || !assetRefForRow) {
            return nullptr;
        }
        const auto ref = assetRefForRow(items.first()->row());
        if (!ref.has_value()) {
            return nullptr;
        }
        auto *mime = new QMimeData;
        mime->setData(QString::fromLatin1(kAssetRefMimeType),
                      QByteArray(reinterpret_cast<const char *>(ref->data()),
                                 static_cast<qsizetype>(ref->size())));
        return mime;
    }
};


QString stateText(uint8_t availability) {
    switch (availability) {
    case 1:
        return QStringLiteral("Managed");
    case 2:
        return QStringLiteral("Missing");
    default:
        return QStringLiteral("External");
    }
}

QString sizeText(const std::optional<uint64_t> &size) {
    if (!size.has_value()) {
        return QStringLiteral("—");
    }
    const uint64_t bytes = *size;
    if (bytes < 1024) {
        return QStringLiteral("%1 B").arg(bytes);
    }
    double value = static_cast<double>(bytes);
    const char *units[] = {"KB", "MB", "GB", "TB"};
    size_t unit = 0;
    do {
        value /= 1024.0;
        ++unit;
    } while (value >= 1024.0 && unit < std::size(units));
    return QStringLiteral("%1 %2").arg(value, 0, 'f', 1).arg(units[unit - 1]);
}

} // namespace

AssetsPanel::AssetsPanel(ClientState *state, QWidget *parent) : QWidget(parent), state(state) {
    setAcceptDrops(true);

    auto *mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(4, 4, 4, 4);

    auto *assetTable = new AssetTableWidget(0, 4, this);
    table = assetTable;
    table->setAcceptDrops(true);
    table->viewport()->setAcceptDrops(true);
    assetTable->dragEnterHandler = [this](QDragEnterEvent *event) { handleDragEnter(event); };
    assetTable->dragMoveHandler = [this](QDragMoveEvent *event) { handleDragMove(event); };
    assetTable->dragLeaveHandler = [this](QDragLeaveEvent *event) { handleDragLeave(event); };
    assetTable->dropHandler = [this](QDropEvent *event) { handleDrop(event); };
    table->setHorizontalHeaderLabels({tr("Name"), tr("State"), tr("Size"), tr("Usage")});
    table->setSelectionBehavior(QAbstractItemView::SelectRows);
    table->setSelectionMode(QAbstractItemView::SingleSelection);
    table->setEditTriggers(QAbstractItemView::NoEditTriggers);
    table->setDragEnabled(true);
    table->setDragDropMode(QAbstractItemView::DragOnly);
    assetTable->assetRefForRow = [this](int row) -> std::optional<std::array<uint8_t, 16>> {
        if (row < 0 || row >= entries.size()) {
            return std::nullopt;
        }
        const AssetEntry &entry = entries[row];
        if (entry.availability == 2) {
            // Missing は実体が無いので配置できない。
            return std::nullopt;
        }
        return entry.id;
    };
    table->verticalHeader()->setVisible(false);
    table->horizontalHeader()->setSectionResizeMode(0, QHeaderView::Stretch);
    mainLayout->addWidget(table);

    emptyHintLabel = new QLabel(tr("Drop files here to register them as assets"), this);
    emptyHintLabel->setAlignment(Qt::AlignCenter);
    mainLayout->addWidget(emptyHintLabel);

    auto *buttonLayout = new QHBoxLayout();
    renameButton = new QPushButton(tr("Rename"));
    removeButton = new QPushButton(tr("Remove"));
    relinkButton = new QPushButton(tr("Relink..."));
    buttonLayout->addWidget(renameButton);
    buttonLayout->addWidget(removeButton);
    buttonLayout->addWidget(relinkButton);
    buttonLayout->addStretch();
    mainLayout->addLayout(buttonLayout);

    connect(table, &QTableWidget::itemSelectionChanged, this, &AssetsPanel::updateButtonStates);
    connect(renameButton, &QPushButton::clicked, this, &AssetsPanel::onRenameClicked);
    connect(removeButton, &QPushButton::clicked, this, &AssetsPanel::onRemoveClicked);
    connect(relinkButton, &QPushButton::clicked, this, &AssetsPanel::onRelinkClicked);

    updateButtonStates();
    reload();
}

void AssetsPanel::reload() {
    auto result = Assets::list(state);
    if (result.isError()) {
        return;
    }
    entries = result.unwrap();
    emptyHintLabel->setVisible(entries.isEmpty());

    table->setRowCount(entries.size());
    for (int row = 0; row < entries.size(); ++row) {
        const AssetEntry &entry = entries[row];
        auto *nameItem = new QTableWidgetItem(entry.name);
        nameItem->setToolTip(entry.path);
        table->setItem(row, 0, nameItem);
        table->setItem(row, 1, new QTableWidgetItem(stateText(entry.availability)));
        auto *sizeItem = new QTableWidgetItem(sizeText(entry.fingerprintSize));
        sizeItem->setTextAlignment(Qt::AlignRight | Qt::AlignVCenter);
        table->setItem(row, 2, sizeItem);
        table->setItem(row, 3,
                       new QTableWidgetItem(entry.inUse ? QString() : tr("Unused")));
    }
    updateButtonStates();
}

void AssetsPanel::dragEnterEvent(QDragEnterEvent *event) {
    handleDragEnter(event);
}

void AssetsPanel::dragMoveEvent(QDragMoveEvent *event) {
    handleDragMove(event);
}

void AssetsPanel::dragLeaveEvent(QDragLeaveEvent *event) {
    handleDragLeave(event);
    QWidget::dragLeaveEvent(event);
}

void AssetsPanel::dropEvent(QDropEvent *event) {
    handleDrop(event);
}

bool AssetsPanel::canRegisterDrop(const QMimeData *mimeData) const {
    if (!mimeData || mimeData->hasFormat(QString::fromLatin1(kAssetRefMimeType))) {
        return false;
    }
    return !extractDropFilePaths(mimeData).isEmpty();
}

void AssetsPanel::handleDragEnter(QDragEnterEvent *event) {
    if (canRegisterDrop(event->mimeData())) {
        setDropHighlighted(true);
        event->acceptProposedAction();
    } else {
        event->ignore();
    }
}

void AssetsPanel::handleDragMove(QDragMoveEvent *event) {
    if (canRegisterDrop(event->mimeData())) {
        setDropHighlighted(true);
        event->acceptProposedAction();
    } else {
        event->ignore();
    }
}

void AssetsPanel::handleDragLeave(QDragLeaveEvent *event) {
    Q_UNUSED(event);
    setDropHighlighted(false);
}

void AssetsPanel::handleDrop(QDropEvent *event) {
    setDropHighlighted(false);
    const QStringList paths = canRegisterDrop(event->mimeData())
                                  ? extractDropFilePaths(event->mimeData())
                                  : QStringList{};
    if (paths.isEmpty()) {
        event->ignore();
        return;
    }
    event->acceptProposedAction();
    registerDroppedFiles(paths);
}

void AssetsPanel::registerDroppedFiles(const QStringList &paths) {
    QStringList failed;
    for (const QString &path : paths) {
        if (!state->requests().registerAssetReference(path).has_value()) {
            failed.append(path);
        }
    }
    if (!failed.isEmpty()) {
        QMessageBox::warning(this, tr("Register Asset"),
                             tr("Failed to register the following files:\n%1").arg(failed.join(QStringLiteral("\n"))));
    }
}

void AssetsPanel::setDropHighlighted(bool highlighted) {
    if (dropHighlighted == highlighted) {
        return;
    }
    dropHighlighted = highlighted;
    if (highlighted) {
        const QString color = palette().color(QPalette::Highlight).name();
        table->setStyleSheet(QStringLiteral("QTableWidget { border: 2px solid %1; }").arg(color));
    } else {
        table->setStyleSheet(QString());
    }
}

const AssetEntry *AssetsPanel::selectedEntry() const {
    const int row = table->currentRow();
    if (row < 0 || row >= entries.size()) {
        return nullptr;
    }
    return &entries[row];
}

void AssetsPanel::updateButtonStates() {
    const AssetEntry *entry = selectedEntry();
    renameButton->setEnabled(entry != nullptr);
    removeButton->setEnabled(entry != nullptr);
    relinkButton->setEnabled(entry != nullptr && entry->availability == 2);
}

void AssetsPanel::onRenameClicked() {
    const AssetEntry *entry = selectedEntry();
    if (!entry) {
        return;
    }
    bool ok = false;
    const QString name = QInputDialog::getText(this, tr("Rename Asset"), tr("Name:"),
                                               QLineEdit::Normal, entry->name, &ok);
    if (!ok || name.isEmpty() || name == entry->name) {
        return;
    }
    state->requests().renameAsset(entry->id, name);
}

void AssetsPanel::onRemoveClicked() {
    const AssetEntry *entry = selectedEntry();
    if (!entry) {
        return;
    }
    if (entry->inUse) {
        QMessageBox::warning(this, tr("Remove Asset"),
                             tr("This asset is used by one or more clips and cannot be removed."));
        return;
    }
    const auto answer = QMessageBox::question(
        this, tr("Remove Asset"),
        tr("Remove \"%1\" from the project? The file itself is not deleted.").arg(entry->name),
        QMessageBox::Yes | QMessageBox::No, QMessageBox::No);
    if (answer != QMessageBox::Yes) {
        return;
    }
    state->requests().removeAsset(entry->id);
}

void AssetsPanel::onRelinkClicked() {
    const AssetEntry *entry = selectedEntry();
    if (!entry || entry->availability != 2) {
        return;
    }
    const QString startDir = QFileInfo(entry->path).absolutePath();
    const QString path = QFileDialog::getOpenFileName(this, tr("Relink Asset"), startDir);
    if (path.isEmpty()) {
        return;
    }
    state->requests().relinkAsset(entry->id, path);
}

} // namespace esotereel::window
