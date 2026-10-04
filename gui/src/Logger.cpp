#include "esotereel_gui_helper.h"
#include "ffi/StringView.h"
#include <QDebug>
#include <QHash>
#include <QMessageLogContext>
#include <QReadLocker>
#include <QReadWriteLock>
#include <QString>
#include <QThread>
#include <QWriteLocker>
#include <cstdio>
#include <cstdlib>
#include <utility>

namespace esotereel {
namespace {

thread_local bool isRustLog = false;
thread_local size_t rustLogLevel = 0;
thread_local QString rustLogTarget;
QReadWriteLock logSettingsLock;
size_t defaultQtLogLevel = 3;
QHash<QString, size_t> qtTargetLogLevels;

int parseLevel(const QString &name, int fallback) {
    if (name.compare("Off", Qt::CaseInsensitive) == 0)
        return 0;
    if (name.compare("Error", Qt::CaseInsensitive) == 0)
        return 1;
    if (name.compare("Warn", Qt::CaseInsensitive) == 0 || name.compare("Warning", Qt::CaseInsensitive) == 0)
        return 2;
    if (name.compare("Info", Qt::CaseInsensitive) == 0)
        return 3;
    if (name.compare("Debug", Qt::CaseInsensitive) == 0)
        return 4;
    if (name.compare("Trace", Qt::CaseInsensitive) == 0)
        return 5;
    return fallback;
}

size_t qtMessageLevel(QtMsgType type) {
    switch (type) {
    case QtDebugMsg:
        return 4;
    case QtInfoMsg:
        return 3;
    case QtWarningMsg:
        return 2;
    case QtCriticalMsg:
    case QtFatalMsg:
        return 1;
    }
    return 3;
}

QString levelName(QtMsgType type) {
    switch (type) {
    case QtDebugMsg:
        return "DEBUG";
    case QtInfoMsg:
        return "INFO";
    case QtWarningMsg:
        return "WARN";
    case QtCriticalMsg:
        return "ERROR";
    case QtFatalMsg:
        return "FATAL";
    }
    return "LOG";
}

QString rustLevelName(size_t level) {
    switch (level) {
    case 1:
        return "ERROR";
    case 2:
        return "WARN";
    case 3:
        return "INFO";
    case 4:
        return "DEBUG";
    case 5:
        return "TRACE";
    default:
        return "LOG";
    }
}

QString shortFunctionName(const char *function) {
    QString name = QString::fromUtf8(function);
    const qsizetype argumentsStart = name.indexOf('(');
    if (argumentsStart >= 0) {
        name.truncate(argumentsStart);
    }

    const qsizetype lastScope = name.lastIndexOf("::");
    if (lastScope >= 0) {
        const qsizetype previousScope = name.lastIndexOf("::", lastScope - 1);
        if (previousScope >= 0) {
            name = name.mid(previousScope + 2);
        }
    }

    const qsizetype returnTypeEnd = name.lastIndexOf(' ');
    if (returnTypeEnd >= 0) {
        name = name.mid(returnTypeEnd + 1);
    }
    return name;
}

void qtMessageHandler(QtMsgType type, const QMessageLogContext &context, const QString &message) {
    QString threadName = QThread::currentThread()->objectName();
    if (threadName.isEmpty()) {
        threadName = "unnamed";
    }
    threadName.replace('\n', ' ');

    const QString level = isRustLog ? rustLevelName(rustLogLevel) : levelName(type);
    const QString category = isRustLog ? rustLogTarget
                                       : (context.category && *context.category ? QString::fromUtf8(context.category)
                                                                                : QStringLiteral("default"));

    if (type != QtFatalMsg) {
        QReadLocker lock(&logSettingsLock);
        const size_t threshold = qtTargetLogLevels.value(category, defaultQtLogLevel);
        const size_t messageLevel = isRustLog ? rustLogLevel : qtMessageLevel(type);
        if (threshold == 0 || messageLevel > threshold) {
            return;
        }
    }

    QString source;
    if (!isRustLog && context.function) {
        source = shortFunctionName(context.function);
    }

    const QString sourceField = source.isEmpty() ? QString() : QString(" [%1]").arg(source);
    const QString formatted =
        QString("[%1] [thread_name=%2] [%3]%4 %5").arg(level, threadName, category, sourceField, message);
    const QByteArray utf8 = formatted.toUtf8();
    std::fprintf(stderr, "%s\n", utf8.constData());
    std::fflush(stderr);

    if (type == QtFatalMsg) {
        std::abort();
    }
}

struct RustLogScope {
    RustLogScope(size_t level, const QString &target) {
        isRustLog = true;
        rustLogLevel = level;
        rustLogTarget = target;
    }

    ~RustLogScope() {
        isRustLog = false;
        rustLogTarget.clear();
    }
};

} // namespace

void installQtMessageHandler() {
    qInstallMessageHandler(qtMessageHandler);
}

void setQtLogSettings(const QString &defaultLevel, const QHash<QString, QString> &targetLevels) {
    QHash<QString, size_t> parsedTargetLevels;
    for (auto it = targetLevels.cbegin(); it != targetLevels.cend(); ++it) {
        const int level = parseLevel(it.value(), -1);
        if (level >= 0) {
            parsedTargetLevels.insert(it.key(), static_cast<size_t>(level));
        }
    }

    QWriteLocker lock(&logSettingsLock);
    defaultQtLogLevel = static_cast<size_t>(parseLevel(defaultLevel, 3));
    qtTargetLogLevels = std::move(parsedTargetLevels);
}

void qtLogCallback(size_t level, esotereel_gui_helper::FfiStringView target_view,
                   esotereel_gui_helper::FfiStringView msg_view) {
    QString target = StringView::toQString(target_view);
    QString message = StringView::toQString(msg_view);

    const RustLogScope logScope(level, target);

    switch (level) {
    case 1:
        qCritical().noquote() << message;
        break;
    case 2:
        qWarning().noquote() << message;
        break;
    case 3:
        qInfo().noquote() << message;
        break;
    default:
        qDebug().noquote() << message;
        break;
    }
}

} // namespace esotereel