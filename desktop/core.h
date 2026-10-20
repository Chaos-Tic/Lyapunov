#pragma once
#include <QtCore>

// The desktop talks to the Rust core (`lyapunov`) through subprocesses only:
// arguments in, one JSON document out (or JSON lines while resuming).
// LYAPUNOV_DEMO_DIR serves canned answers for documentation captures, so they
// never show anyone's real cards or windows.
namespace core {

inline QString path() {
    if (const QString forced = qEnvironmentVariable("LYAPUNOV_CORE"); !forced.isEmpty()) return forced;
    QString name = "lyapunov";
#ifdef Q_OS_WIN
    name += ".exe";
#endif
    const QString sibling = QCoreApplication::applicationDirPath() + "/" + name;
    if (QFile::exists(sibling)) return sibling;
    return QStandardPaths::findExecutable(name);
}

// The core needs no injected libraries (an AppImage may set some), and the
// GNOME extension refuses a caller that has them.
inline QProcessEnvironment environment() {
    QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    for (const char *name : {"LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT"}) env.remove(name);
    return env;
}

struct Result {
    QJsonDocument doc;
    QString error;
    bool ok() const { return error.isEmpty(); }
    QJsonObject obj() const { return doc.object(); }
};

inline Result parse(const QByteArray &out) {
    const QList<QByteArray> lines = out.trimmed().split('\n');
    if (lines.isEmpty() || lines.last().trimmed().isEmpty()) return {{}, "no answer"};
    QJsonParseError e{};
    const QJsonDocument doc = QJsonDocument::fromJson(lines.last().trimmed(), &e);
    if (e.error != QJsonParseError::NoError) return {{}, e.errorString()};
    if (doc.isObject() && doc.object().contains("error")) return {doc, doc.object().value("error").toString()};
    return {doc, {}};
}

inline Result demo(const QStringList &args) {
    const QDir dir(qEnvironmentVariable("LYAPUNOV_DEMO_DIR"));
    QFile f(dir.filePath(args.value(0) + ".json"));
    if (!f.open(QIODevice::ReadOnly)) return {{}, {}};
    return parse(f.readAll());
}

// Runs a command and waits for its JSON answer.
inline Result run(const QStringList &args, const QByteArray &input = {}, int timeoutMs = 20000) {
    if (qEnvironmentVariableIsSet("LYAPUNOV_DEMO_DIR")) return demo(args);
    const QString exe = path();
    if (exe.isEmpty()) return {{}, "core-missing"};
    QProcess p;
    p.setProcessEnvironment(environment());
    p.start(exe, QStringList(args) << "--json");
    if (!p.waitForStarted(5000)) return {{}, "core-missing"};
    if (!input.isEmpty()) p.write(input);
    p.closeWriteChannel();
    if (!p.waitForFinished(timeoutMs)) {
        p.kill(); p.waitForFinished(1000);
        return {{}, "the core did not answer in time"};
    }
    Result r = parse(p.readAllStandardOutput());
    if (r.ok() && p.exitCode() != 0) r.error = QString::fromUtf8(p.readAllStandardError()).trimmed();
    return r;
}

} // namespace core
