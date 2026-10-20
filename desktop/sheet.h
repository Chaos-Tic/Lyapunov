#pragma once
#include <QtWidgets>
#include <functional>
#include "glass.h"

// A sheet: a glass panel over the dimmed window, for capturing, editing,
// resuming and confirming. It holds real widgets, so keyboard navigation and
// assistive technologies work as everywhere else. Escape closes it.
class GlassPanel : public QFrame {
public:
    using QFrame::QFrame;
protected:
    void paintEvent(QPaintEvent *) override {
        QPainter p(this); p.setRenderHint(QPainter::Antialiasing);
        const glass::Theme th = glass::theme();
        const QRectF r = QRectF(rect()).adjusted(14, 10, -14, -18);
        glass::shadow(p, r, 28, th.shadow, 1.0, 14);
        // Sheets sit over other glass: a denser body keeps their text crisp.
        p.setPen(Qt::NoPen); p.setBrush(glass::opaque() ? th.surface : glass::alpha(th.surface, th.dark ? .9 : .86));
        p.drawRoundedRect(r, 28, 28);
        glass::surface(p, r, 28, th, glass::tint("lilac").base, .9, .12);
    }
};

class Sheet : public QWidget {
public:
    QVBoxLayout *body;
    QLabel *heading;
    std::function<void()> onClose;

    Sheet(QWidget *parent, const QString &title, int maxWidth = 580) : QWidget(parent), widest(maxWidth) {
        setAttribute(Qt::WA_DeleteOnClose);
        auto *outer = new QVBoxLayout(this); outer->setContentsMargins(16, 16, 16, 16);
        auto *row = new QHBoxLayout; row->addStretch();
        panel = new GlassPanel; panel->setObjectName("sheet");
        row->addWidget(panel); row->addStretch();
        outer->addStretch(1); outer->addLayout(row); outer->addStretch(2);
        auto *inner = new QVBoxLayout(panel); inner->setContentsMargins(42, 34, 42, 40); inner->setSpacing(14);
        heading = new QLabel(title); heading->setObjectName("title"); heading->setFont(glass::font(21, QFont::DemiBold));
        heading->setWordWrap(true); heading->setTextFormat(Qt::PlainText);
        inner->addWidget(heading);
        scroller = new QScrollArea; scroller->setWidgetResizable(true); scroller->setFrameShape(QFrame::NoFrame);
        scroller->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        auto *content = new QWidget; body = new QVBoxLayout(content); body->setContentsMargins(0, 0, 6, 0); body->setSpacing(10);
        scroller->setWidget(content);
        inner->addWidget(scroller, 1);
        buttons = new QHBoxLayout; buttons->setSpacing(10); buttons->addStretch();
        inner->addLayout(buttons);
        setAccessibleName(title);
    }

    QPushButton *addButton(const QString &text, bool primary, std::function<void()> fn) {
        auto *b = new QPushButton(text);
        if (primary) { b->setObjectName("primary"); b->setDefault(true); b->setAutoDefault(true); }
        buttons->addWidget(b);
        QObject::connect(b, &QPushButton::clicked, this, [fn] { if (fn) fn(); });
        return b;
    }
    void clearButtons() {
        while (buttons->count() > 1) {
            QLayoutItem *item = buttons->takeAt(1);
            if (item->widget()) item->widget()->deleteLater();
            delete item;
        }
    }
    void clearBody() {
        while (QLayoutItem *item = body->takeAt(0)) {
            if (item->widget()) item->widget()->deleteLater();
            else if (item->layout()) clearLayout(item->layout());
            delete item;
        }
    }
    void open() {
        setGeometry(parentWidget()->rect()); fit(); show(); raise();
        if (QWidget *first = firstFocus()) first->setFocus(Qt::OtherFocusReason);
    }
    void dismiss() {
        if (onClose) onClose();
        close();
    }
    void fit() {
        const int w = qMin(widest, parentWidget()->width() - 32);
        panel->setFixedWidth(w);
        body->parentWidget()->setFixedWidth(w - 84 - 6);
        const int bodyHeight = body->parentWidget()->sizeHint().height();
        const int room = parentWidget()->height() - 32 - 74 - heading->sizeHint().height() - 60;
        scroller->setFixedHeight(qMax(80, qMin(bodyHeight + 4, room)));
    }

protected:
    GlassPanel *panel;
    QScrollArea *scroller;
    QHBoxLayout *buttons;
    int widest;

    static void clearLayout(QLayout *l) {
        while (QLayoutItem *item = l->takeAt(0)) {
            if (item->widget()) item->widget()->deleteLater();
            else if (item->layout()) clearLayout(item->layout());
            delete item;
        }
    }
    QWidget *firstFocus() {
        for (QWidget *w : panel->findChildren<QWidget *>()) {
            if (w->focusPolicy() & Qt::TabFocus && w->isEnabled() && (qobject_cast<QLineEdit *>(w) || qobject_cast<QCheckBox *>(w))) return w;
        }
        for (QWidget *w : panel->findChildren<QPushButton *>()) if (static_cast<QPushButton *>(w)->isDefault()) return w;
        return nullptr;
    }
    void paintEvent(QPaintEvent *) override {
        QPainter p(this); p.fillRect(rect(), glass::theme().scrim);
    }
    void resizeEvent(QResizeEvent *) override { fit(); }
    void keyPressEvent(QKeyEvent *e) override {
        if (e->key() == Qt::Key_Escape) { dismiss(); return; }
        QWidget::keyPressEvent(e);
    }
    void mousePressEvent(QMouseEvent *e) override {
        // A click on the dimmed area outside the panel closes the sheet.
        if (!panel->geometry().contains(e->position().toPoint())) dismiss();
    }
};

inline QLabel *label(const QString &text, const char *name = "", bool wrap = true) {
    auto *l = new QLabel(text); l->setObjectName(name); l->setTextFormat(Qt::PlainText); l->setWordWrap(wrap);
    return l;
}

inline QLabel *section(const QString &text) {
    auto *l = label(text.toUpper(), "section"); l->setFont(glass::font(11.5, QFont::DemiBold));
    return l;
}

// Seven swatches with their names: the name of the chosen colour is shown,
// so colour is never the only cue.
class TintPicker : public QWidget {
public:
    QString value;
    explicit TintPicker(const QString &initial, QWidget *parent = nullptr) : QWidget(parent), value(initial) {
        auto *row = new QHBoxLayout(this); row->setContentsMargins(0, 0, 0, 0); row->setSpacing(6);
        auto *group = new QButtonGroup(this); group->setExclusive(true);
        for (const auto &t : glass::tints()) {
            auto *b = new QToolButton; b->setObjectName("swatch"); b->setCheckable(true); b->setAutoRaise(true);
            QPixmap pm(22, 22); pm.fill(Qt::transparent);
            { QPainter p(&pm); p.setRenderHint(QPainter::Antialiasing);
              QLinearGradient g(0, 0, 22, 22); g.setColorAt(0, t.lite); g.setColorAt(1, t.base);
              p.setPen(QPen(glass::alpha(Qt::black, .12), 1)); p.setBrush(g); p.drawEllipse(QRectF(1, 1, 20, 20)); }
            b->setIcon(QIcon(pm)); b->setIconSize(QSize(22, 22));
            b->setToolTip(L(t.label)); b->setAccessibleName(L(t.label));
            b->setChecked(t.id == value);
            group->addButton(b); row->addWidget(b);
            connect(b, &QToolButton::toggled, this, [this, t](bool on) { if (on) { value = t.id; name->setText(L(t.label)); } });
        }
        name = label(L(glass::tint(value).label), "muted", false);
        row->addSpacing(8); row->addWidget(name); row->addStretch();
    }
private:
    QLabel *name;
};

// Folders and documents kept with a card: a list with Remove buttons, two Add
// buttons, and drag and drop from the file manager.
class PlaceList : public QWidget {
public:
    QVector<QPair<QString, bool>> places;
    explicit PlaceList(const QVector<QPair<QString, bool>> &initial, QWidget *parent = nullptr) : QWidget(parent), places(initial) {
        setAcceptDrops(true);
        auto *v = new QVBoxLayout(this); v->setContentsMargins(0, 0, 0, 0); v->setSpacing(6);
        rows = new QVBoxLayout; rows->setSpacing(4); v->addLayout(rows);
        hint = label(L("Drop folders or documents here"), "hint");
        v->addWidget(hint);
        auto *add = new QHBoxLayout; add->setSpacing(8);
        auto *folder = new QPushButton(L("Add a folder…")); auto *file = new QPushButton(L("Add a document…"));
        add->addWidget(folder); add->addWidget(file); add->addStretch(); v->addLayout(add);
        connect(folder, &QPushButton::clicked, this, [this] {
            const QString d = QFileDialog::getExistingDirectory(this, L("Add a folder…"), QDir::homePath());
            if (!d.isEmpty()) addPath(d);
        });
        connect(file, &QPushButton::clicked, this, [this] {
            for (const QString &f : QFileDialog::getOpenFileNames(this, L("Add a document…"), QDir::homePath())) addPath(f);
        });
        rebuild();
    }
    void addPath(const QString &path) {
        const QFileInfo fi(path);
        if (!fi.exists()) return;
        const QString abs = QDir::cleanPath(fi.absoluteFilePath());
        for (const auto &p : places) if (p.first == abs) return;
        places.append({QDir::toNativeSeparators(abs), fi.isDir()});
        rebuild();
    }
    QJsonArray json() const {
        QJsonArray a;
        for (const auto &p : places) a.append(QJsonObject{{"path", p.first}, {"kind", p.second ? "folder" : "file"}});
        return a;
    }
protected:
    void dragEnterEvent(QDragEnterEvent *e) override { if (e->mimeData()->hasUrls()) e->acceptProposedAction(); }
    void dropEvent(QDropEvent *e) override {
        for (const QUrl &u : e->mimeData()->urls()) if (u.isLocalFile()) addPath(u.toLocalFile());
        e->acceptProposedAction();
    }
private:
    QVBoxLayout *rows;
    QLabel *hint;
    void rebuild() {
        while (QLayoutItem *item = rows->takeAt(0)) { if (item->widget()) item->widget()->deleteLater(); delete item; }
        for (int i = 0; i < places.size(); ++i) {
            auto *row = new QWidget; auto *h = new QHBoxLayout(row); h->setContentsMargins(0, 0, 0, 0);
            auto *name = label(QString(places[i].second ? "▸ " : "· ") + places[i].first, "", false);
            name->setToolTip(places[i].first);
            name->setMinimumWidth(0); name->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
            auto *remove = new QPushButton(L("Remove")); remove->setObjectName("flat");
            remove->setAccessibleName(L("Remove") + " " + places[i].first);
            h->addWidget(name, 1); h->addWidget(remove);
            rows->addWidget(row);
            const QString path = places[i].first;
            connect(remove, &QPushButton::clicked, this, [this, path] {
                for (int k = 0; k < places.size(); ++k) if (places[k].first == path) { places.remove(k); break; }
                rebuild();
            });
        }
        hint->setVisible(places.isEmpty());
        if (auto *s = qobject_cast<QWidget *>(parentWidget())) s->updateGeometry();
    }
};
