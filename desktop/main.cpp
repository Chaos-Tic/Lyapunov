#include <QtWidgets>
#include <QLocalServer>
#include <QLocalSocket>
#include "cardstack.h"
#include "core.h"
#include "sheet.h"

// Lyapunov's window: the card stack over a soft environment, a header with
// Capture and the menu, and sheets for capturing, editing and resuming.
// Every change goes through the core (`lyapunov`), which validates it.


// Notes and reasons come from the core in English; known ones are translated.
static QString translateNote(const QString &note) {
    static const QStringList known{"not installed", "not found", "runs in a terminal", "window closed",
        "this window cannot be moved", "a window could not be moved", "windows are not arranged on this desktop",
        "started, but no window appeared in time to be arranged",
        "Lyapunov cannot start this application (no menu entry)", "programs and scripts are not opened from a card",
        "network locations are not opened", "no longer a folder", "no longer a document"};
    if (known.contains(note)) return L(note);
    if (!lyapunov::french()) return note;
    static const QList<QPair<QRegularExpression, QString>> patterns{
        {QRegularExpression("^(\\d+) saved window\\(s\\) not reopened$"), "%1 fenêtre(s) enregistrée(s) non rouverte(s)"},
        {QRegularExpression("^screen (.+) is not connected; moved to (.+)$"), "l'écran %1 n'est pas branché ; déplacée vers %2"},
        {QRegularExpression("^desktop (.+) no longer exists; left on its current desktop$"), "le bureau %1 n'existe plus ; laissée sur son bureau actuel"},
        {QRegularExpression("^workspace (.+) will be created$"), "l'espace de travail %1 sera créé"},
        {QRegularExpression("^tiled: .*$"), "en mosaïque : le gestionnaire de fenêtres choisit sa place"},
    };
    for (const auto &[re, fr] : patterns) {
        const QRegularExpressionMatch m = re.match(note);
        if (!m.hasMatch()) continue;
        QString out = fr;
        for (int i = 1; i <= m.lastCapturedIndex(); ++i) out = out.arg(m.captured(i));
        return out;
    }
    return note;
}

class Logo : public QWidget {
public:
    Logo() { setFixedSize(44, 44); setAccessibleName("Lyapunov"); }
protected:
    void paintEvent(QPaintEvent *) override { QPainter p(this); glass::logo(p, QRectF(rect())); }
};

class Window : public QWidget {
public:
    CardStack *stack;
    QJsonObject doctor;

    Window() {
        setWindowTitle("Lyapunov");
        setWindowIcon(glass::appIcon());
        resize(820, 760); setMinimumSize(520, 560);
        auto *v = new QVBoxLayout(this); v->setContentsMargins(28, 22, 28, 18); v->setSpacing(12);
        auto *header = new QHBoxLayout; header->setSpacing(12);
        header->addWidget(new Logo);
        auto *titles = new QVBoxLayout; titles->setSpacing(0);
        auto *name = label("Lyapunov", "title", false); name->setFont(glass::font(22, QFont::DemiBold));
        auto *sub = label(L("Your spaces"), "muted", false); sub->setFont(glass::font(13));
        titles->addWidget(name); titles->addWidget(sub);
        header->addLayout(titles); header->addStretch();
        capture = new QPushButton(L("Capture this space"));
        capture->setIcon(plusIcon()); capture->setShortcut(QKeySequence::New);
        header->addWidget(capture);
        menuButton = new QToolButton; menuButton->setText("⋯"); menuButton->setObjectName("menu"); menuButton->setAccessibleName(L("Menu"));
        menuButton->setToolTip(L("Menu")); menuButton->setPopupMode(QToolButton::InstantPopup);
        menuButton->setFixedSize(46, 36); menuButton->setFont(glass::font(18, QFont::DemiBold));
        menuButton->setMenu(buildMenu());
        header->addWidget(menuButton);
        v->addLayout(header);

        noticeRow = new QWidget; auto *n = new QHBoxLayout(noticeRow); n->setContentsMargins(4, 0, 4, 0);
        notice = label("", "muted"); noticeAction = new QPushButton; noticeAction->setObjectName("flat");
        n->addWidget(notice, 1); n->addWidget(noticeAction);
        noticeRow->hide();
        v->addWidget(noticeRow);

        stack = new CardStack;
        v->addWidget(stack, 1);
        stack->resumeRequested = [this](int i) { startResume(i); };
        stack->menuRequested = [this](int i, QPoint at) { cardMenu(i)->popup(at); };
        stack->deleteRequested = [this](int i) { confirmDelete(i); };
        stack->editRequested = [this](int i) { edit(i); };
        stack->captureRequested = [this] { startCapture(); };
        connect(capture, &QPushButton::clicked, this, [this] { startCapture(); });
        setStyleSheet(glass::styleSheet(glass::theme()));
        setTabOrder(stack, capture); setTabOrder(capture, menuButton);
        stack->setFocus();
    }

    void load(const QString &select = {}) {
        const core::Result r = core::run({"list"});
        if (!r.ok()) { fail(r.error); return; }
        QVector<CardView> list;
        for (const auto &c : r.obj()["cards"].toArray()) list << CardView::from(c.toObject());
        problems = r.obj()["problems"].toArray();
        stack->setCards(list, select);
        refreshNotice();
    }

    void diagnose() {
        const core::Result d = core::run({"doctor"});
        if (d.ok()) doctor = d.obj();
        refreshNotice();
    }

    // Run from an AppImage: its folder changes at every start, so the GNOME
    // extension is told again which executable to answer, and the menu entry
    // is offered when there is none.
    void appImageSetUp() {
        if (!qEnvironmentVariableIsSet("APPIMAGE")) return;
        if (qEnvironmentVariable("XDG_CURRENT_DESKTOP").contains("GNOME", Qt::CaseInsensitive))
            core::run({"gnome-extension", "--trust"});
        const QString entry = QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation) + "/applications/lyapunov.desktop";
        if (QFile::exists(entry) || !noticeRow->isHidden()) return;
        notice->setText(L("Add Lyapunov to the applications menu?"));
        noticeAction->setText(L("Add")); noticeAction->show(); noticeAction->disconnect();
        connect(noticeAction, &QPushButton::clicked, this, [this] {
            const core::Result r = core::run({"integrate"});
            if (!r.ok()) { fail(r.error); return; }
            noticeRow->hide();
        });
        noticeRow->show();
    }

    // Opens straight on a sheet (from the command line or the launcher's action).
    void startCapture() { captureSheet(nullptr); }

    // Documentation captures: the window in each state, from canned answers.
    bool render(const QString &dir) {
        QDir().mkpath(dir);
        auto shot = [&](const QString &name) {
            QCoreApplication::processEvents(); QCoreApplication::processEvents();
            return grab().save(QDir(dir).filePath(name + ".png"));
        };
        bool ok = true;
        load(); stack->select(1);
        ok &= shot("stack");
        captureSheet(nullptr); ok &= shot("capture"); closeSheet();
        startResume(1); ok &= shot("plan");
        showReport(core::run({"report"}).obj()); ok &= shot("report"); closeSheet();
        stack->setCards({}); ok &= shot("empty");
        return ok;
    }

    // Frames of the stack moving from card to card, at 30 per second, for
    // the README animation.
    bool renderFrames(const QString &dir) {
        QDir().mkpath(dir);
        load(); stack->rendering = true;
        int n = 0;
        auto frame = [&] { QCoreApplication::processEvents(); return grab().save(QDir(dir).filePath(QString("%1.png").arg(n++, 4, 10, QChar('0')))); };
        auto hold = [&](int count) { bool ok = true; for (int i = 0; i < count; ++i) ok &= frame(); return ok; };
        bool ok = hold(24);
        for (int target : {1, 2, 3, 4, 2, 0}) {
            stack->selectAnimated(target);
            for (int i = 0; i < 90 && stack->advance(1.0 / 30); ++i) ok &= frame();
            ok &= hold(target == 0 ? 30 : 20);
        }
        return ok;
    }

protected:
    QPushButton *capture, *noticeAction;
    QToolButton *menuButton;
    QWidget *noticeRow;
    QLabel *notice;
    QJsonArray problems;
    QPointer<Sheet> sheet;
    QPixmap backdrop;
    bool backdropDark = false, styledDark = glass::theme().dark;
    QProcess *resuming = nullptr;

    static QIcon plusIcon() {
        QPixmap pm(32, 32); pm.fill(Qt::transparent);
        QPainter p(&pm); p.setRenderHint(QPainter::Antialiasing);
        p.setPen(QPen(glass::theme().ink, 2.6, Qt::SolidLine, Qt::RoundCap)); glass::glyph(p, "plus", QPointF(16, 16), 1.2);
        return QIcon(pm);
    }

    void paintEvent(QPaintEvent *) override {
        const glass::Theme th = glass::theme();
        if (backdrop.isNull() || backdrop.deviceIndependentSize().toSize() != size() || backdropDark != th.dark) {
            backdrop = glass::environment(size(), devicePixelRatioF(), th); backdropDark = th.dark;
        }
        QPainter p(this); p.drawPixmap(0, 0, backdrop);
    }
    void resizeEvent(QResizeEvent *) override { if (sheet) sheet->setGeometry(rect()); }
    // The system switched between light and dark: restyle once. A style
    // sheet changes the palette itself, so palette events are not followed.
    void changeEvent(QEvent *e) override {
        QWidget::changeEvent(e);
        if (e->type() != QEvent::ThemeChange && e->type() != QEvent::ApplicationPaletteChange) return;
        const bool dark = glass::theme().dark;
        if (dark == styledDark) return;
        styledDark = dark; backdrop = {};
        setStyleSheet(glass::styleSheet(glass::theme())); update();
    }

    void fail(const QString &error) {
        Sheet *s = openSheet(L("Something went wrong"));
        s->body->addWidget(label(error == "core-missing" ? L("The Lyapunov core (lyapunov) was not found. Reinstall Lyapunov.") : error));
        s->addButton(L("Close"), true, [this] { closeSheet(); });
        s->open();
    }

    Sheet *openSheet(const QString &title, int width = 580) {
        closeSheet();
        sheet = new Sheet(this, title, width);
        sheet->onClose = [this] { if (resuming) return; stack->setFocus(); };
        return sheet;
    }
    void closeSheet() {
        if (sheet) { sheet->onClose = nullptr; sheet->close(); sheet = nullptr; }
        stack->setFocus();
    }

    void refreshNotice() {
        noticeRow->hide(); noticeAction->hide(); noticeAction->disconnect();
        if (!problems.isEmpty()) {
            notice->setText(L("%1 card(s) could not be read").arg(problems.size()));
            noticeAction->setText(L("Details")); noticeAction->show();
            connect(noticeAction, &QPushButton::clicked, this, [this] { showProblems(); });
            noticeRow->show();
            return;
        }
        if (doctor.isEmpty() || doctor["backend"].toString() != "launch-only") return;
        const QString desktop = doctor["session"].toObject()["desktop"].toString();
        if (desktop.contains("GNOME", Qt::CaseInsensitive)) {
            notice->setText(L("On GNOME, arranging windows needs the Lyapunov extension."));
            noticeAction->setText(L("Set up GNOME integration")); noticeAction->show();
            connect(noticeAction, &QPushButton::clicked, this, [this] { setUpGnome(); });
        } else {
            notice->setText(L("Windows are not arranged on this desktop: applications, folders and documents still reopen."));
        }
        noticeRow->show();
    }

    void setUpGnome() {
        const core::Result r = core::run({"gnome-extension"});
        if (!r.ok()) { fail(r.error); return; }
        if (r.obj()["running"].toBool()) { diagnose(); return; }
        Sheet *s = openSheet(L("Set up GNOME integration"));
        s->body->addWidget(label(L("Log out and back in to finish setting up GNOME integration.")));
        s->addButton(L("Close"), true, [this] { closeSheet(); });
        s->open();
    }

    // --- menus ------------------------------------------------------------

    QMenu *buildMenu() {
        auto *m = new QMenu(this);
        m->addAction(L("Import cards…"), this, [this] { importCards(); });
        m->addAction(L("Export all cards…"), this, [this] { exportCards({}); });
        m->addAction(L("Open the cards folder"), this, [this] {
            if (doctor.isEmpty()) diagnose();
            QDesktopServices::openUrl(QUrl::fromLocalFile(doctor["data_dir"].toString()));
        });
        m->addSeparator();
        QMenu *lang = m->addMenu(L("Language"));
        auto *group = new QActionGroup(lang);
        const QString chosen = lyapunov::settings().value("Language", "auto").toString();
        for (const auto &[id, text] : QList<QPair<QString, QString>>{{"auto", L("Automatic")}, {"en", "English"}, {"fr", "Français"}}) {
            QAction *a = lang->addAction(text); a->setCheckable(true); a->setChecked(id == chosen); group->addAction(a);
            connect(a, &QAction::triggered, this, [this, id] {
                lyapunov::settings().setValue("Language", id); lyapunov::settings().sync();
                Sheet *s = openSheet(L("Language"));
                s->body->addWidget(label(L("The language changes the next time Lyapunov opens.")));
                s->addButton(L("Close"), true, [this] { closeSheet(); });
                s->open();
            });
        }
        for (const auto &[key, text] : QList<QPair<QString, QString>>{{"ReduceMotion", L("Reduce motion")}, {"ReduceTransparency", L("Reduce transparency")}}) {
            QAction *a = m->addAction(text); a->setCheckable(true); a->setChecked(lyapunov::settings().value(key, false).toBool());
            connect(a, &QAction::toggled, this, [this, key](bool on) {
                lyapunov::settings().setValue(key, on); lyapunov::settings().sync(); update(); stack->update();
            });
        }
        m->addSeparator();
        m->addAction(L("Diagnostics"), this, [this] { showDiagnostics(); });
        m->addAction(L("About Lyapunov"), this, [this] { showAbout(); });
        return m;
    }

    QMenu *cardMenu(int i) {
        auto *m = new QMenu(this); m->setAttribute(Qt::WA_DeleteOnClose);
        m->addAction(L("Edit…"), this, [this, i] { edit(i); });
        m->addAction(L("Update from what is open…"), this, [this, i] { captureSheet(&stack->all()[i]); });
        m->addAction(L("Duplicate"), this, [this, i] {
            const core::Result r = core::run({"duplicate", stack->all()[i].id});
            if (!r.ok()) { fail(r.error); return; }
            load(r.obj()["id"].toString());
        });
        QAction *up = m->addAction(L("Move up"), this, [this, i] { move(i, -1); });
        QAction *down = m->addAction(L("Move down"), this, [this, i] { move(i, +1); });
        up->setEnabled(i > 0); down->setEnabled(i + 1 < stack->all().size());
        m->addAction(L("Export…"), this, [this, i] { exportCards({stack->all()[i].id}); });
        m->addSeparator();
        m->addAction(L("Delete…"), this, [this, i] { confirmDelete(i); });
        return m;
    }

    void move(int i, int delta) {
        QStringList ids;
        for (const auto &c : stack->all()) ids << c.id;
        const int j = i + delta;
        if (j < 0 || j >= ids.size()) return;
        ids.swapItemsAt(i, j);
        const QString keep = ids[j];
        const core::Result r = core::run(QStringList{"reorder"} + ids);
        if (!r.ok()) { fail(r.error); return; }
        load(keep);
    }

    // --- capture and update -----------------------------------------------

    void captureSheet(const CardView *existing) {
        QStringList args{"scan", "--exclude-pid", QString::number(QCoreApplication::applicationPid())};
        if (existing) args << "--compare" << existing->id;
        const core::Result r = core::run(args);
        if (!r.ok()) { fail(r.error); return; }
        const QJsonObject scan = r.obj();
        const QString id = existing ? existing->id : QString();
        Sheet *s = openSheet(existing ? L("Update “%1”").arg(existing->name) : L("Capture this space"), 620);

        if (existing) {
            const QJsonObject d = scan["compare"].toObject();
            QStringList lines;
            auto list = [&](const char *key, const QString &title) {
                QStringList names; for (const auto &x : d[key].toArray()) names << x.toString();
                if (!names.isEmpty()) lines << title + " : " + names.join(", ");
            };
            list("added_apps", L("Added")); list("removed_apps", L("No longer open")); list("moved_apps", L("Changed"));
            s->body->addWidget(label(lines.isEmpty() ? L("Nothing changed since the card was saved.") : lines.join("\n"), "muted"));
        }

        s->body->addWidget(section(L("Name")));
        auto *name = new QLineEdit(existing ? existing->name : QString());
        name->setPlaceholderText(L("Name this activity")); name->setMaxLength(80); name->setAccessibleName(L("Name"));
        s->body->addWidget(name);
        auto *error = label("", "muted"); error->hide(); s->body->addWidget(error);
        s->body->addWidget(section(L("Colour")));
        const int count = stack->all().size();
        auto *tint = new TintPicker(existing ? existing->tint : glass::tints()[count % glass::tints().size()].id);
        s->body->addWidget(tint);

        s->body->addWidget(section(L("Applications and windows")));
        struct Pick { QCheckBox *box; QJsonObject app; QLineEdit *workspace; };
        auto picks = std::make_shared<QVector<Pick>>();
        const QJsonArray apps = scan["apps"].toArray();
        if (apps.isEmpty()) s->body->addWidget(label(L("No application window is open."), "muted"));
        for (const auto &a : apps) {
            const QJsonObject c = a.toObject(), app = c["app"].toObject();
            const int n = app["windows"].toArray().size();
            auto *box = new QCheckBox(QString("%1 — %2").arg(app["name"].toString(), n == 1 ? L("1 window") : L("%1 windows").arg(n)));
            box->setChecked(true);
            s->body->addWidget(box);
            QStringList titles; for (const auto &t : c["titles"].toArray()) if (!t.toString().isEmpty()) titles << t.toString();
            QString hint = titles.join(" · ");
            if (!c["launchable"].toBool()) hint += (hint.isEmpty() ? "" : "\n") + L("Lyapunov cannot restart it; its windows are arranged when it is open.");
            if (!hint.isEmpty()) {
                auto *h = label(hint, "hint"); h->setContentsMargins(30, 0, 0, 4); h->setFont(glass::font(12.5));
                s->body->addWidget(h);
            }
            QLineEdit *workspace = nullptr;
            if (app["desktop"].toString() == "code.desktop") {
                auto *row = new QWidget; auto *layout = new QHBoxLayout(row);
                layout->setContentsMargins(30, 0, 0, 5);
                workspace = new QLineEdit;
                workspace->setPlaceholderText(L("VS Code project folder (optional)"));
                workspace->setAccessibleName(L("VS Code project folder (optional)"));
                if (existing) for (const auto &old : existing->raw["apps"].toArray()) {
                    const QJsonObject item = old.toObject();
                    if (item["desktop"].toString() == "code.desktop") workspace->setText(item["workspace"].toString());
                }
                auto *browse = new QPushButton(L("Choose…"));
                connect(browse, &QPushButton::clicked, this, [workspace, this] {
                    const QString folder = QFileDialog::getExistingDirectory(this, L("VS Code project folder"), workspace->text());
                    if (!folder.isEmpty()) workspace->setText(folder);
                });
                layout->addWidget(workspace); layout->addWidget(browse);
                s->body->addWidget(row);
            }
            picks->append({box, app, workspace});
        }
        if (!scan["limitation"].toString().isEmpty())
            s->body->addWidget(label(L("Windows are not arranged on this desktop: applications, folders and documents still reopen."), "hint"));
        s->body->addWidget(label(L("Window titles are shown here to help you choose. They are not saved.") + " " +
            L("Nothing is moved or closed while capturing."), "hint"));

        s->body->addWidget(section(L("Folders and documents")));
        auto *places = new PlaceList(existing ? existing->places : QVector<QPair<QString, bool>>{});
        s->body->addWidget(places);

        s->addButton(L("Cancel"), false, [this] { closeSheet(); });
        s->addButton(existing ? L("Replace the card") : L("Save card"), true, [=] {
            if (name->text().trimmed().isEmpty()) {
                error->setText(L("Give the card a name.")); error->show(); name->setFocus(); return;
            }
            QJsonArray chosen; for (const auto &p : *picks) if (p.box->isChecked()) {
                QJsonObject app = p.app;
                if (p.workspace && !p.workspace->text().trimmed().isEmpty()) {
                    app["workspace"] = p.workspace->text().trimmed();
                    app["windows"] = QJsonArray{};
                }
                chosen.append(app);
            }
            QJsonObject draft{{"name", name->text().trimmed()}, {"tint", tint->value}, {"apps", chosen}, {"places", places->json()}};
            if (existing) {
                draft["id"] = id; draft["note"] = existing->note;
                draft["resume"] = existing->raw["resume"];
            }
            const core::Result saved = core::run({"save"}, QJsonDocument(draft).toJson(QJsonDocument::Compact));
            if (!saved.ok()) { error->setText(saved.error); error->show(); return; }
            closeSheet(); load(saved.obj()["id"].toString());
        });
        s->open();
        name->setFocus();
    }

    void edit(int i) {
        const CardView c = stack->all()[i];
        Sheet *s = openSheet(L("Edit “%1”").arg(c.name));
        s->body->addWidget(section(L("Name")));
        auto *name = new QLineEdit(c.name); name->setMaxLength(80); name->setAccessibleName(L("Name"));
        s->body->addWidget(name);
        s->body->addWidget(section(L("Note")));
        auto *note = new QLineEdit(c.note); note->setMaxLength(280); note->setAccessibleName(L("Note"));
        note->setPlaceholderText(L("A few words about this activity (optional)"));
        s->body->addWidget(note);
        auto *error = label("", "muted"); error->hide(); s->body->addWidget(error);
        s->body->addWidget(section(L("Colour")));
        auto *tint = new TintPicker(c.tint); s->body->addWidget(tint);
        s->body->addWidget(section(L("Folders and documents")));
        auto *places = new PlaceList(c.places); s->body->addWidget(places);
        auto *workspaceSection = section(L("VS Code project"));
        s->body->addWidget(workspaceSection);
        auto *workspace = new QLineEdit;
        workspace->setPlaceholderText(L("VS Code project folder (optional)"));
        workspace->setAccessibleName(L("VS Code project folder (optional)"));
        bool hasCode = false;
        for (const auto &v : c.raw["apps"].toArray()) if (v.toObject()["desktop"].toString() == "code.desktop") {
            workspace->setText(v.toObject()["workspace"].toString()); hasCode = true; break;
        }
        workspaceSection->setVisible(hasCode);
        workspace->setVisible(hasCode);
        auto *browse = new QPushButton(L("Choose…")); browse->setVisible(hasCode);
        connect(browse, &QPushButton::clicked, this, [workspace, this] {
            const QString folder = QFileDialog::getExistingDirectory(this, L("VS Code project folder"), workspace->text());
            if (!folder.isEmpty()) workspace->setText(folder);
        });
        s->body->addWidget(workspace); s->body->addWidget(browse);
        s->body->addWidget(section(L("Resume")));
        auto *ask = new QCheckBox(L("Show what will happen before resuming")); ask->setChecked(c.ask);
        auto *arrange = new QCheckBox(L("Move windows that are already open")); arrange->setChecked(c.arrange);
        s->body->addWidget(ask); s->body->addWidget(arrange);
        s->addButton(L("Cancel"), false, [this] { closeSheet(); });
        s->addButton(L("Save"), true, [=] {
            if (name->text().trimmed().isEmpty()) { error->setText(L("Give the card a name.")); error->show(); return; }
            QJsonArray apps;
            for (const auto &v : c.raw["apps"].toArray()) {
                QJsonObject app = v.toObject();
                if (app["desktop"].toString() == "code.desktop") {
                    if (workspace->text().trimmed().isEmpty()) app.remove("workspace");
                    else { app["workspace"] = workspace->text().trimmed(); app["windows"] = QJsonArray{}; }
                }
                apps.append(app);
            }
            QJsonObject draft{{"id", c.id}, {"name", name->text().trimmed()}, {"note", note->text().trimmed()}, {"tint", tint->value},
                {"apps", apps}, {"places", places->json()},
                {"resume", QJsonObject{{"ask", ask->isChecked()}, {"arrange_open_windows", arrange->isChecked()}}}};
            const core::Result saved = core::run({"save"}, QJsonDocument(draft).toJson(QJsonDocument::Compact));
            if (!saved.ok()) { error->setText(saved.error); error->show(); return; }
            closeSheet(); load(c.id);
        });
        s->open();
    }

    void confirmDelete(int i) {
        const CardView c = stack->all()[i];
        Sheet *s = openSheet(L("Delete “%1”?").arg(c.name), 520);
        s->body->addWidget(label(L("Only the card is deleted. The applications, folders and documents it lists are not touched.")));
        s->addButton(L("Cancel"), true, [this] { closeSheet(); });
        s->addButton(L("Delete"), false, [this, c] {
            const core::Result r = core::run({"delete", c.id});
            closeSheet();
            if (!r.ok()) { fail(r.error); return; }
            load();
        });
        s->open();
    }

    // --- resume -----------------------------------------------------------

    void startResume(int i) {
        const CardView c = stack->all()[i];
        // Moving an already open window can move another project's work.
        // Always show the plan when that option is enabled.
        if (!c.ask && !c.arrange) { runResume(c, {}, false); return; }
        const core::Result r = core::run({"plan", c.id});
        if (!r.ok()) { fail(r.error); return; }
        const QJsonObject plan = r.obj();
        Sheet *s = openSheet(L("Resume “%1”").arg(c.name), 600);
        struct Step { QCheckBox *box; QString key; };
        auto steps = std::make_shared<QVector<Step>>();
        if (!plan["limitation"].toString().isEmpty())
            s->body->addWidget(label(L("Windows are not arranged on this desktop: applications, folders and documents still reopen."), "hint"));
        bool hasUnverifiedApp = false, hasVsCodeProject = false;
        for (const auto &v : plan["steps"].toArray()) {
            const QJsonObject st = v.toObject();
            if (st["kind"].toString() == "running" && st["reason"].toString() != "VS Code project verified") hasUnverifiedApp = true;
            if (st["key"].toString().startsWith("app:") && st.contains("path")) hasVsCodeProject = true;
        }
        if (hasUnverifiedApp)
            s->body->addWidget(label(L("Lyapunov recognises the app, not its project or document. Open windows may belong to another space; nothing is closed."), "hint"));
        if (hasVsCodeProject)
            s->body->addWidget(label(L("VS Code project windows are verified by the local extension and never moved automatically."), "hint"));
        const bool canArrange = plan["capabilities"].toObject()["geometry"].toBool();
        for (const auto &v : plan["steps"].toArray()) {
            const QJsonObject st = v.toObject();
            const QString kind = st["kind"].toString();
            QString what;
            if (kind == "launch") what = L("Will be started");
            else if (kind == "running") what = st["reason"].toString() == "VS Code project verified" ? L("Open — project verified") : L("Open — project not verified");
            else if (kind == "open") what = L("Will be opened");
            else what = L("Unavailable") + " — " + translateNote(st["reason"].toString());
            QString title = st["name"].toString();
            if (st.contains("path")) {
                const QString path = st["path"].toString();
                title += "   " + QDir::toNativeSeparators(st["key"].toString().startsWith("app:") ? path : QFileInfo(path).path());
            }
            auto *box = new QCheckBox(title);
            box->setChecked(kind != "unavailable"); box->setEnabled(kind != "unavailable");
            box->setAccessibleName(title + ", " + what);
            s->body->addWidget(box);
            auto *status = label(what, "hint"); status->setContentsMargins(30, 0, 0, 6); status->setFont(glass::font(12.5));
            s->body->addWidget(status);
            steps->append({box, st["key"].toString()});
        }
        auto *arrange = new QCheckBox(L("Also move windows that are already open")); arrange->setChecked(c.arrange);
        arrange->setVisible(canArrange);
        s->body->addSpacing(4); s->body->addWidget(arrange);
        auto *error = label("", "muted"); error->hide(); s->body->addWidget(error);
        s->addButton(L("Cancel"), false, [this] { closeSheet(); });
        s->addButton(L("Resume"), true, [=] {
            QStringList only; for (const auto &st : *steps) if (st.box->isChecked()) only << st.key;
            if (only.isEmpty()) { error->setText(L("Nothing selected.")); error->show(); return; }
            runResume(c, only, arrange->isChecked());
        });
        s->open();
    }

    void runResume(const CardView &c, const QStringList &only, bool arrange) {
        Sheet *s = openSheet(L("Resume “%1”").arg(c.name), 560);
        auto *status = label(L("Resuming…")); status->setFont(glass::font(15));
        s->body->addWidget(status);
        s->body->addWidget(label(L("Stopping here leaves what has already started open."), "hint"));
        resuming = new QProcess(this);
        QStringList args{"resume", c.id, "--json", arrange ? "--arrange" : "--no-arrange"};
        for (const QString &k : only) args << "--only" << k;
        auto buffer = std::make_shared<QByteArray>();
        connect(resuming, &QProcess::readyReadStandardOutput, this, [this, status, buffer] {
            buffer->append(resuming->readAllStandardOutput());
            while (true) {
                const int nl = buffer->indexOf('\n');
                if (nl < 0) break;
                const QJsonObject e = QJsonDocument::fromJson(buffer->left(nl)).object();
                buffer->remove(0, nl + 1);
                const QString ev = e["event"].toString();
                if (ev == "starting") status->setText(L("Starting %1…").arg(e["name"].toString()));
                else if (ev == "opening") status->setText(L("Opening %1…").arg(e["name"].toString()));
                else if (ev == "waiting") status->setText(L("Waiting for windows…"));
                else if (ev == "arranging") status->setText(L("Arranging %1 windows…").arg(e["windows"].toInt()));
                else if (ev == "report") { finishResume(e); return; }
                else if (e.contains("error")) { finishResume({}); fail(e["error"].toString()); return; }
            }
        });
        connect(resuming, &QProcess::finished, this, [this] {
            if (resuming) { resuming->deleteLater(); resuming = nullptr; }
        });
        s->addButton(L("Stop waiting"), false, [this] {
            if (resuming) { resuming->kill(); resuming->deleteLater(); resuming = nullptr; }
            closeSheet();
        });
        s->open();
        const QString exe = core::path();
        if (exe.isEmpty()) { fail("core-missing"); return; }
        resuming->setProcessEnvironment(core::environment());
        resuming->start(exe, args);
    }

    void finishResume(const QJsonObject &report) {
        if (resuming) { resuming->disconnect(); resuming->deleteLater(); resuming = nullptr; }
        if (!report.isEmpty()) showReport(report);
    }

    void showReport(const QJsonObject &report) {
        Sheet *s = openSheet(L("“%1” resumed").arg(report["name"].toString()), 600);
        for (const auto &v : report["outcomes"].toArray()) {
            const QJsonObject o = v.toObject();
            const QString st = o["status"].toString();
            static const QHash<QString, QPair<QString, QString>> look{
                {"window-found", {"check", "Window found"}}, {"app-launched", {"start", "Application started"}},
                {"left-open", {"app", "Already open"}}, {"opened", {"check", "Opened"}},
                {"unavailable", {"missing", "Unavailable"}}, {"failed", {"fail", "Failed"}}, {"skipped", {"skip", "Left out"}}};
            const auto [icon, text] = look.value(st, {"app", st});
            auto *row = new QWidget; auto *h = new QHBoxLayout(row); h->setContentsMargins(0, 2, 0, 2); h->setSpacing(12);
            auto *mark = new QLabel; mark->setFixedSize(28, 28);
            QPixmap pm(56, 56); pm.setDevicePixelRatio(2); pm.fill(Qt::transparent);
            { QPainter p(&pm); p.setRenderHint(QPainter::Antialiasing);
              const glass::Theme th = glass::theme();
              const QColor tone = st == "failed" ? QColor("#d9534f") : st == "unavailable" ? th.ink2 : st == "skipped" ? th.faint : th.ink;
              p.setPen(QPen(tone, 2, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin)); glass::glyph(p, icon, QPointF(14, 14), 1.0); }
            mark->setPixmap(pm);
            auto *texts = new QVBoxLayout; texts->setSpacing(1);
            QString line = o["name"].toString() + " — " + L(text);
            const int windows = o["windows"].toInt();
            if (windows > 0 && st != "skipped" && st != "unavailable")
                line += " · " + L("%1 of %2 windows placed").arg(o["placed"].toInt()).arg(windows);
            auto *main = label(line); texts->addWidget(main);
            QStringList notes; for (const auto &n : o["notes"].toArray()) notes << translateNote(n.toString());
            if (!notes.isEmpty()) texts->addWidget(label(notes.join("\n"), "hint"));
            h->addWidget(mark, 0, Qt::AlignTop); h->addLayout(texts, 1);
            row->setAccessibleName(line);
            s->body->addWidget(row);
        }
        s->addButton(L("Back to cards"), false, [this] { closeSheet(); });
        s->addButton(L("Close Lyapunov"), true, [] { qApp->quit(); });
        s->open();
    }

    // --- import, export, information ---------------------------------------

    void exportCards(const QStringList &ids) {
        const QString file = QFileDialog::getSaveFileName(this, L("Export…"),
            QDir::home().filePath(ids.size() == 1 ? "lyapunov-card.json" : "lyapunov-cards.json"), L("Lyapunov cards (*.json)"));
        if (file.isEmpty()) return;
        const core::Result r = core::run(QStringList{"export"} + ids + QStringList{"--output", file});
        if (!r.ok()) { fail(r.error); return; }
        Sheet *s = openSheet(L("Export…"));
        s->body->addWidget(label(L("Exported to %1").arg(QDir::toNativeSeparators(file))));
        s->addButton(L("Close"), true, [this] { closeSheet(); });
        s->open();
    }

    void importCards() {
        const QString file = QFileDialog::getOpenFileName(this, L("Import cards…"), QDir::homePath(), L("Lyapunov cards (*.json)"));
        if (file.isEmpty()) return;
        const core::Result r = core::run({"import", file, "--dry-run"});
        if (!r.ok()) { fail(r.error); return; }
        Sheet *s = openSheet(L("Import “%1”").arg(QFileInfo(file).fileName()), 600);
        s->body->addWidget(label(L("Imported cards never start anything until you resume them."), "hint"));
        bool any = false;
        for (const auto &v : r.obj()["cards"].toArray()) {
            const QJsonObject c = v.toObject();
            if (c.contains("error")) {
                s->body->addWidget(label(L("Rejected") + " — " + c["name"].toString() + "\n" + c["error"].toString(), "muted"));
                continue;
            }
            any = true;
            auto *title = label(c["name"].toString(), "title"); s->body->addWidget(title);
            QStringList lines;
            for (const auto &e : c["executables"].toArray()) lines << L("Starts") + " : " + e.toString();
            for (const auto &w : c["workspaces"].toArray()) lines << L("VS Code project") + " : " + QDir::toNativeSeparators(w.toString());
            for (const auto &p : c["places"].toArray()) lines << L("Opens") + " : " + QDir::toNativeSeparators(p.toString());
            if (!lines.isEmpty()) s->body->addWidget(label(lines.join("\n"), "hint"));
        }
        s->addButton(L("Cancel"), false, [this] { closeSheet(); });
        QPushButton *go = s->addButton(L("Import"), true, [this, file] {
            const core::Result done = core::run({"import", file});
            closeSheet();
            if (!done.ok()) { fail(done.error); return; }
            load();
        });
        go->setEnabled(any);
        s->open();
    }

    void showProblems() {
        Sheet *s = openSheet(L("Unreadable cards"), 620);
        s->body->addWidget(label(L("These files were left untouched. Fix or remove them in the cards folder."), "hint"));
        for (const auto &v : problems) {
            const QJsonObject p = v.toObject();
            s->body->addWidget(label(p["file"].toString() + "\n" + p["error"].toString()));
        }
        s->addButton(L("Open the cards folder"), false, [this] {
            if (doctor.isEmpty()) diagnose();
            QDesktopServices::openUrl(QUrl::fromLocalFile(doctor["data_dir"].toString() + "/cards"));
        });
        s->addButton(L("Close"), true, [this] { closeSheet(); });
        s->open();
    }

    void showDiagnostics() {
        diagnose();
        Sheet *s = openSheet(L("Diagnostics"), 600);
        const QJsonObject caps = doctor["capabilities"].toObject();
        const QJsonObject session = doctor["session"].toObject();
        QStringList lines{
            "Lyapunov " + doctor["version"].toString(),
            QString("%1 · %2 · %3").arg(session["os"].toString(), session["session_type"].toString(), session["desktop"].toString()),
            "backend: " + doctor["backend"].toString(),
            QString("list %1 · move %2 · desktops %3 · states %4").arg(caps["list"].toBool()).arg(caps["geometry"].toBool())
                .arg(caps["desktops"].toBool()).arg(caps["states"].toBool()),
            QString("screens %1 · desktops %2 · windows %3").arg(doctor["outputs"].toInt()).arg(doctor["desktops"].toInt()).arg(doctor["windows"].toInt()),
            "data: " + QDir::toNativeSeparators(doctor["data_dir"].toString()),
        };
        if (!doctor["limitation"].toString().isEmpty()) lines << doctor["limitation"].toString();
        auto *text = label(lines.join("\n")); text->setTextInteractionFlags(Qt::TextSelectableByMouse);
        s->body->addWidget(text);
        s->addButton(L("Close"), true, [this] { closeSheet(); });
        s->open();
    }

    void showAbout() {
        Sheet *s = openSheet(L("About Lyapunov"), 520);
        auto *logo = new Logo; logo->setFixedSize(72, 72);
        s->body->addWidget(logo, 0, Qt::AlignLeft);
        s->body->addWidget(label(L("Save a card and pick it up later.")));
        s->body->addWidget(label(L("Version %1. Everything stays on this computer: no account, no network.").arg(LYAPUNOV_VERSION), "hint"));
        s->addButton(L("Close"), true, [this] { closeSheet(); });
        s->open();
    }
};

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    QApplication::setApplicationName("Lyapunov");
    QApplication::setOrganizationName("Lyapunov");
    QApplication::setApplicationVersion(LYAPUNOV_VERSION);
    // The Wayland app ID matches lyapunov.desktop, so the compositor shows the
    // right icon and Lyapunov never captures itself.
    QGuiApplication::setDesktopFileName("lyapunov");
    const QStringList args = app.arguments();
    if (args.contains("--version")) { QTextStream(stdout) << "lyapunov-desktop " << LYAPUNOV_VERSION << Qt::endl; return 0; }

    if (args.contains("--smoke-test")) {
        Window w; w.resize(820, 760); w.load(); w.show(); QCoreApplication::processEvents();
        return w.grab().isNull() ? 1 : 0;
    }
    if (const int i = args.indexOf("--render-frames"); i >= 0) {
        Window w; w.resize(880, 780); w.show(); QCoreApplication::processEvents();
        return w.renderFrames(args.value(i + 1, "frames")) ? 0 : 1;
    }
    if (const int i = args.indexOf("--render"); i >= 0) {
        Window w; w.resize(880, 780); w.show(); QCoreApplication::processEvents();
        return w.render(args.value(i + 1, "renders")) ? 0 : 1;
    }

    // One window per user: a second start brings it forward (and passes
    // --capture along).
    const QString key = "lyapunov-desktop-" + QString::number(qHash(QDir::homePath()));
    {
        QLocalSocket probe; probe.connectToServer(key);
        if (probe.waitForConnected(300)) {
            probe.write(args.contains("--capture") ? "capture\n" : "show\n"); probe.waitForBytesWritten(300);
            return 0;
        }
    }
    QLocalServer::removeServer(key);
    QLocalServer server; server.setSocketOptions(QLocalServer::UserAccessOption); server.listen(key);

    Window w;
    w.show();
    QObject::connect(&server, &QLocalServer::newConnection, &w, [&] {
        QLocalSocket *s = server.nextPendingConnection();
        QObject::connect(s, &QLocalSocket::readyRead, &w, [&w, s] {
            const QByteArray msg = s->readAll().trimmed();
            w.showNormal(); w.raise(); w.activateWindow();
            if (msg == "capture") w.startCapture();
            s->deleteLater();
        });
    });
    QTimer::singleShot(0, &w, [&w, args] {
        w.load(); w.diagnose(); w.appImageSetUp();
        if (args.contains("--capture")) w.startCapture();
    });
    return app.exec();
}
