#include "WidgetManager.h"

namespace esotereel::window {

WidgetManager::WidgetManager(ads::CDockManager* dockManager)
    : dockManager_(dockManager) {}

ads::CDockWidget* WidgetManager::createDock(QWidget* widget, 
                                           const QString& name,
                                           ads::DockWidgetArea area,
                                           ads::CDockWidget* reference,
                                           bool visible) {
    auto* dock = new ads::CDockWidget(dockManager_, name);
    dock->setWidget(widget);
    
    if (reference) {
        dockManager_->addDockWidget(area, dock, reference->dockAreaWidget());
    } else {
        dockManager_->addDockWidget(area, dock);
    }
    
    dock->toggleView(visible);
    docks_[name] = dock;
    return dock;
}

void WidgetManager::addToMenu(QMenu* menu, ads::CDockWidget* dock) {
    menu->addAction(dock->toggleViewAction());
}

ads::CDockWidget* WidgetManager::getDock(const QString& name) const {
    return docks_.value(name, nullptr);
}

} // namespace esotereel::window
