#pragma once
#include <DockManager.h>
#include <QMap>
#include <QString>
#include <QMenu>
#include <utility>

namespace esotereel::window {

class WidgetManager {
public:
    explicit WidgetManager(ads::CDockManager* dockManager);
    
    // ウィジェット作成とdock作成・追加をまとめる
    template<typename T, typename... Args>
    T* createWidget(const QString& name, 
                   ads::DockWidgetArea area, 
                   Args&&... args) {
        T* widget = new T(std::forward<Args>(args)...);
        createDock(widget, name, area, nullptr, true);
        return widget;
    }
    
    // ウィジェット作成とdock作成・追加をまとめる（reference指定版）
    template<typename T, typename... Args>
    T* createWidgetWithReference(const QString& name, 
                                ads::DockWidgetArea area, 
                                ads::CDockWidget* reference,
                                Args&&... args) {
        T* widget = new T(std::forward<Args>(args)...);
        createDock(widget, name, area, reference, true);
        return widget;
    }
    
    // ウィジェット作成とdock作成・追加をまとめる（reference + visible指定版）
    template<typename T, typename... Args>
    T* createWidgetWithReferenceAndVisible(const QString& name, 
                                          ads::DockWidgetArea area, 
                                          ads::CDockWidget* reference,
                                          bool visible,
                                          Args&&... args) {
        T* widget = new T(std::forward<Args>(args)...);
        createDock(widget, name, area, reference, visible);
        return widget;
    }
    
    // 既存のウィジェットからdockを作成・追加
    ads::CDockWidget* createDock(QWidget* widget, 
                                 const QString& name,
                                 ads::DockWidgetArea area,
                                 ads::CDockWidget* reference = nullptr,
                                 bool visible = true);
    
    // メニューにdockの表示/非表示アクションを追加
    void addToMenu(QMenu* menu, ads::CDockWidget* dock);
    
    // 名前でdockを取得
    ads::CDockWidget* getDock(const QString& name) const;
    
private:
    ads::CDockManager* dockManager_;
    QMap<QString, ads::CDockWidget*> docks_;
};

} // namespace esotereel::window
