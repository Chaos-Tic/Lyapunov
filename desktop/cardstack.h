#pragma once
#include <QtWidgets>
#include <functional>
#include "glass.h"

// The card selector: a vertical stack, like cards in a wallet. Every card
// shows its top band (emblem, name, contents); the active one opens fully
// in place with its applications, places and the Resume action, and the
// cards after it continue below. Selecting another card (click, arrow keys,
// wheel, vertical drag) slides the stack on springs. With reduced motion,
// everything snaps.
struct CardView {
    QString id, name, note, tint, created, modified;
    QStringList apps;
    int windows = 0;
    QVector<QPair<QString, bool>> places; // path, is a folder
    bool ask = true, arrange = true;
    QJsonObject raw;

    static CardView from(const QJsonObject &o) {
        CardView c;
        c.raw = o; c.id = o["id"].toString(); c.name = o["name"].toString(); c.note = o["note"].toString();
        c.tint = o["tint"].toString("peach"); c.created = o["created"].toString(); c.modified = o["modified"].toString();
        for (const auto &a : o["apps"].toArray()) {
            c.apps << a.toObject()["name"].toString();
            c.windows += a.toObject()["windows"].toArray().size();
        }
        for (const auto &p : o["places"].toArray())
            c.places.append({p.toObject()["path"].toString(), p.toObject()["kind"].toString() == "folder"});
        const QJsonObject r = o["resume"].toObject();
        c.ask = r["ask"].toBool(true); c.arrange = r["arrange_open_windows"].toBool(true);
        return c;
    }
    QString summary() const {
        QStringList parts;
        if (!apps.isEmpty()) parts << (apps.size() == 1 ? L("1 app") : L("%1 apps").arg(apps.size()));
        if (windows > 0) parts << (windows == 1 ? L("1 window") : L("%1 windows").arg(windows));
        if (!places.isEmpty()) parts << (places.size() == 1 ? L("1 place") : L("%1 places").arg(places.size()));
        return parts.isEmpty() ? L("Nothing saved yet") : parts.join(" · ");
    }
};

inline QString shortDate(const QString &rfc3339) {
    const QDateTime t = QDateTime::fromString(rfc3339, Qt::ISODate);
    if (!t.isValid()) return {};
    return QLocale(lyapunov::french() ? QLocale::French : QLocale::English).toString(t.toLocalTime().date(), "d MMM yyyy");
}

class CardStack : public QWidget {
public:
    static constexpr qreal band = 66, tuck = 30, full = 324, gap = 16, maxWidth = 660, radius = 26;
    std::function<void(int)> resumeRequested, deleteRequested, editRequested;
    std::function<void(int, QPoint)> menuRequested;
    std::function<void()> captureRequested;
    std::function<void(int)> currentChanged;

    CardStack(QWidget *parent = nullptr) : QWidget(parent) {
        setFocusPolicy(Qt::StrongFocus); setMouseTracking(true);
        setAccessibleName(L("Cards"));
        setMinimumHeight(240);
        frames.setInterval(8); frames.setTimerType(Qt::PreciseTimer);
        connect(&frames, &QTimer::timeout, this, [this] { tick(); });
    }

    void setCards(const QVector<CardView> &list, const QString &selectId = {}) {
        const QString keep = selectId.isEmpty() && current >= 0 && current < cards.size() ? cards[current].id : selectId;
        cards = list;
        current = 0;
        for (int i = 0; i < cards.size(); ++i) if (cards[i].id == keep) current = i;
        motion.resize(cards.size());
        relayout(true);
        announce();
        update();
    }
    const QVector<CardView> &all() const { return cards; }
    int currentIndex() const { return cards.isEmpty() ? -1 : current; }
    const CardView *currentCard() const { return cards.isEmpty() ? nullptr : &cards[current]; }

    void select(int i) {
        if (cards.isEmpty()) return;
        i = qBound(0, i, int(cards.size()) - 1);
        if (i == current) return;
        current = i;
        relayout(false); announce(); update();
        if (currentChanged) currentChanged(current);
    }

    // Rectangles at rest, for tests and hit testing (widget coordinates).
    QRectF cardRect(int i) const {
        if (i < 0 || i >= motion.size()) return {};
        const Motion &m = motion[i];
        return QRectF(left() + m.inset.x, m.top.x - scroll.x + topMargin, columnWidth() - 2 * m.inset.x, m.height.x);
    }
    QRectF resumeButton() const { return activeButtons().first; }
    QRectF moreButton() const { return activeButtons().second; }
    QRectF emptyCaptureButton() const {
        const QRectF r = emptyRect();
        return QRectF(r.center().x() - 110, r.bottom() - 74, 220, 44);
    }
    int cardAt(QPointF pos) const {
        for (int i = int(cards.size()) - 1; i >= 0; --i) if (cardRect(i).contains(pos)) return i;
        return -1;
    }

protected:
    struct Motion { glass::Spring top, height, inset, glow; };
    QVector<CardView> cards;
    QVector<Motion> motion;
    glass::Spring scroll;
    int current = 0, hoverButton = 0; // 1 resume, 2 more, 3 empty capture
    bool pressedResume = false, keyboard = false;
public:
    bool rendering = false;
protected:
    QTimer frames;
    QElapsedTimer clock;
    QPointF dragStart{-1, -1};
    int dragFrom = 0;
    qreal wheelAccumulated = 0;
    static constexpr qreal topMargin = 6;

    qreal columnWidth() const { return qMin(maxWidth, qreal(width()) - 32); }
    qreal left() const { return (width() - columnWidth()) / 2; }
    QRectF emptyRect() const {
        const qreal w = columnWidth(), h = qMin(300.0, height() - 24.0);
        return QRectF(left(), qMax(12.0, (height() - h) / 2 - 20), w, h);
    }

    // Targets: each card shows its band; the active card opens fully.
    // The open card is as tall as what it shows.
    qreal openHeight(int i) const {
        const CardView &c = cards[i];
        const qreal w = columnWidth() - 48;
        qreal h = band + 74 + 10;
        if (!c.note.isEmpty()) h += 40;
        if (!c.apps.isEmpty()) {
            const QFontMetricsF fm(glass::font(13, QFont::Medium));
            qreal x = 0; int rows = 1;
            for (const QString &a : c.apps) {
                const qreal cw = qMin(fm.horizontalAdvance(a) + 46, w * .6);
                if (x + cw > w) { if (++rows > 2) { rows = 2; break; } x = 0; }
                x += cw + 8;
            }
            h += 4 + rows * 36 + 6;
        }
        h += qMin(int(c.places.size()), 3) * 26;
        return qBound(band + 96.0, h, full);
    }

    void relayout(bool snap) {
        qreal y = 0, activeTop = 0;
        for (int i = 0; i < cards.size(); ++i) {
            Motion &m = motion[i];
            const bool active = i == current;
            m.top.target = y;
            const qreal open = active ? openHeight(i) : 0;
            m.height.target = active ? open : band + tuck;
            m.inset.target = active ? 0 : 12;
            m.glow.target = active ? 1 : 0;
            if (active) activeTop = y;
            y += active ? open + gap : band;
        }
        const qreal total = y + (current == cards.size() - 1 ? 0 : tuck);
        const qreal view = height() - topMargin - 8;
        // Keep the active card in view, with up to two bands above it.
        const qreal wanted = activeTop - qMin(current, 2) * band;
        scroll.target = qBound(0.0, wanted, qMax(0.0, total - view));
        const bool still = snap || glass::reducedMotion() || (!isVisible() && !rendering);
        if (still) {
            for (Motion &m : motion) { m.top.snap(m.top.target); m.height.snap(m.height.target); m.inset.snap(m.inset.target); m.glow.snap(m.glow.target); }
            scroll.snap(scroll.target);
        } else if (!frames.isActive()) { clock.restart(); frames.start(); }
    }

    void tick() { advance(qMin(qreal(clock.restart()), 40.0) / 1000.0); }

public:
    // Moves the springs by dt seconds; false once everything is at rest.
    // Documentation renders call it with a fixed step for even frames.
    bool advance(qreal dt) {
        bool moving = scroll.step(dt, 300, 34);
        for (Motion &m : motion) {
            moving |= m.top.step(dt, 320, 34);
            moving |= m.height.step(dt, 320, 34);
            moving |= m.inset.step(dt, 320, 34);
            moving |= m.glow.step(dt, 260, 30);
        }
        update();
        if (!moving) frames.stop();
        return moving;
    }
    // Animated selection even where the window is not shown (renders).
    void selectAnimated(int i) {
        if (cards.isEmpty()) return;
        current = qBound(0, i, int(cards.size()) - 1);
        relayout(false); frames.stop(); announce(); update();
    }

protected:

    void resizeEvent(QResizeEvent *) override { relayout(true); }

    QPair<QRectF, QRectF> activeButtons() const {
        if (cards.isEmpty()) return {};
        const QRectF r = cardRect(current);
        const QRectF resume(r.left() + 22, r.bottom() - 62, 220, 42);
        const QRectF more(r.right() - 22 - 96, r.bottom() - 62, 96, 42);
        return {resume, more};
    }

    void announce() {
        if (cards.isEmpty()) { setAccessibleDescription(L("No cards yet")); return; }
        const CardView &c = cards[current];
        setAccessibleName(QString("%1, %2, %3").arg(c.name, L("Card %1 of %2").arg(current + 1).arg(cards.size()), L("selected")));
        setAccessibleDescription(c.summary() + ". " + L("Press Enter to resume, the menu key for more."));
        QAccessibleEvent name(this, QAccessible::NameChanged); QAccessible::updateAccessibility(&name);
        QAccessibleEvent desc(this, QAccessible::DescriptionChanged); QAccessible::updateAccessibility(&desc);
    }

    // --- painting ---------------------------------------------------------

    void paintEvent(QPaintEvent *) override {
        QPainter p(this); p.setRenderHint(QPainter::Antialiasing); p.setRenderHint(QPainter::TextAntialiasing);
        const glass::Theme th = glass::theme();
        if (cards.isEmpty()) { paintEmpty(p, th); return; }
        p.setClipRect(rect());
        for (int i = 0; i < cards.size(); ++i) paintCard(p, th, i);
    }

    void paintEmpty(QPainter &p, const glass::Theme &th) {
        const QRectF r = emptyRect();
        glass::shadow(p, r, radius, th.shadow, .8);
        glass::surface(p, r, radius, th, glass::tint("lilac").base, .6, .25);
        glass::logo(p, QRectF(r.center().x() - 36, r.top() + 28, 72, 72));
        p.setPen(th.ink); p.setFont(glass::font(20, QFont::DemiBold));
        p.drawText(QRectF(r.left() + 24, r.top() + 108, r.width() - 48, 30), Qt::AlignHCenter | Qt::AlignVCenter, L("No cards yet"));
        p.setPen(th.ink2); p.setFont(glass::font(14));
        p.drawText(QRectF(r.left() + 48, r.top() + 142, r.width() - 96, 66), Qt::AlignHCenter | Qt::AlignTop | Qt::TextWordWrap,
            L("A card keeps the applications, windows, folders and documents of one activity. Capture what is open now, and come back to it later."));
        paintCapsule(p, th, emptyCaptureButton(), L("Capture this space"), "plus", true, hoverButton == 3);
        if (hasFocus() && keyboard) {
            p.setPen(QPen(th.focus, 2)); p.setBrush(Qt::NoBrush);
            p.drawRoundedRect(emptyCaptureButton().adjusted(-4, -4, 4, 4), 25, 25);
        }
    }

    void paintCapsule(QPainter &p, const glass::Theme &th, const QRectF &r, const QString &text, const QString &icon,
        bool primary, bool hover, bool pressed = false) {
        p.save();
        const qreal h = r.height();
        if (primary) {
            QColor fill = th.dark ? QColor("#9fdcf8") : QColor("#151820");
            if (hover) fill = th.dark ? fill.lighter(108) : QColor("#2a2f3c");
            if (pressed) fill = fill.darker(115);
            p.setPen(Qt::NoPen); p.setBrush(fill); p.drawRoundedRect(r, h / 2, h / 2);
            p.setPen(QPen(th.dark ? QColor("#10131c") : Qt::white, 1.7, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
        } else {
            glass::surface(p, r, h / 2, th, Qt::white, hover ? 1 : .5);
            p.setPen(QPen(th.ink, 1.7, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
        }
        QFont f = glass::font(14.5, primary ? QFont::DemiBold : QFont::Medium);
        const qreal tw = QFontMetricsF(f).horizontalAdvance(text);
        const qreal iw = icon.isEmpty() ? 0 : 22;
        const qreal x0 = r.center().x() - (tw + iw) / 2;
        if (!icon.isEmpty()) glass::glyph(p, icon, QPointF(x0 + 8, r.center().y()), .95);
        p.setFont(f);
        p.drawText(QRectF(x0 + iw, r.top(), tw + 2, h), Qt::AlignVCenter | Qt::AlignLeft, text);
        p.restore();
    }

    void paintCard(QPainter &p, const glass::Theme &th, int i) {
        const CardView &c = cards[i];
        const Motion &m = motion[i];
        const QRectF r = cardRect(i);
        if (r.bottom() < -20 || r.top() > height() + 20) return;
        const glass::Tint &t = glass::tint(c.tint);
        const qreal glow = m.glow.x;
        glass::shadow(p, r, radius, th.shadow, .35 + .65 * glow, 8 + 8 * glow);
        glass::surface(p, r, radius, th, t.base, .25 + .75 * glow, .55 + .2 * glow);
        // A coloured edge on the left keeps the tint visible on every band.
        p.save();
        QPainterPath clip; clip.addRoundedRect(r, radius, radius); p.setClipPath(clip);
        QLinearGradient edge(r.topLeft(), r.topLeft() + QPointF(0, band));
        edge.setColorAt(0, glass::alpha(t.base, th.dark ? .55 : .5)); edge.setColorAt(1, glass::alpha(t.base, 0));
        p.fillRect(QRectF(r.left(), r.top(), r.width(), band), edge);
        p.restore();

        // Band: emblem, name, contents.
        glass::emblem(p, QPointF(r.left() + 36, r.top() + band / 2), 17, t, th);
        const qreal textLeft = r.left() + 66;
        p.setPen(th.ink); p.setFont(glass::font(17.5, QFont::DemiBold));
        const QString summary = c.summary();
        QFont small = glass::font(13);
        const qreal sw = qMin(QFontMetricsF(small).horizontalAdvance(summary) + 4, r.width() * .42);
        p.drawText(QRectF(textLeft, r.top(), r.width() - (textLeft - r.left()) - sw - 30, band), Qt::AlignVCenter | Qt::AlignLeft,
            QFontMetricsF(p.font()).elidedText(c.name, Qt::ElideRight, r.width() - (textLeft - r.left()) - sw - 30));
        p.setPen(th.ink2); p.setFont(small);
        p.drawText(QRectF(r.right() - sw - 22, r.top(), sw, band), Qt::AlignVCenter | Qt::AlignRight,
            QFontMetricsF(small).elidedText(summary, Qt::ElideRight, sw));

        if (glow > .02) paintDetails(p, th, c, r, glow);
        if (i == current && hasFocus() && keyboard) {
            p.setPen(QPen(th.focus, 2)); p.setBrush(Qt::NoBrush);
            p.drawRoundedRect(r.adjusted(-3, -3, 3, 3), radius + 3, radius + 3);
        }
    }

    void paintDetails(QPainter &p, const glass::Theme &th, const CardView &c, const QRectF &r, qreal glow) {
        p.save();
        p.setOpacity(qBound(0.0, (glow - .25) / .75, 1.0));
        QPainterPath clip; clip.addRoundedRect(r, radius, radius); p.setClipPath(clip);
        qreal y = r.top() + band;
        const qreal x = r.left() + 24, w = r.width() - 48;
        if (!c.note.isEmpty()) {
            p.setPen(th.ink2); p.setFont(glass::font(13.5));
            const QRectF nr(x, y, w, 38);
            p.drawText(nr, Qt::AlignLeft | Qt::AlignTop | Qt::TextWordWrap, c.note);
            y += 40;
        }
        // Applications as capsules, two rows at most.
        QFont chip = glass::font(13, QFont::Medium);
        const QFontMetricsF fm(chip);
        qreal cx = x, cy = y + 4;
        int shown = 0, row = 0;
        for (const QString &a : c.apps) {
            const qreal cw = qMin(fm.horizontalAdvance(a) + 46, w * .6);
            if (cx + cw > x + w) {
                if (++row >= 2) break;
                cx = x; cy += 36;
            }
            const QRectF cr(cx, cy, cw, 30);
            p.setPen(Qt::NoPen); p.setBrush(th.dark ? glass::rgba(255, 255, 255, .10) : glass::rgba(255, 255, 255, .55));
            p.drawRoundedRect(cr, 15, 15);
            p.setPen(QPen(th.ink2, 1.3)); glass::glyph(p, "app", QPointF(cr.left() + 17, cr.center().y()), .8);
            p.setPen(th.ink); p.setFont(chip);
            p.drawText(cr.adjusted(31, 0, -10, 0), Qt::AlignVCenter | Qt::AlignLeft, fm.elidedText(a, Qt::ElideRight, cw - 41));
            cx += cw + 8; ++shown;
        }
        if (shown < c.apps.size()) {
            p.setPen(th.ink2); p.setFont(chip);
            p.drawText(QRectF(cx, cy, 120, 30), Qt::AlignVCenter | Qt::AlignLeft, L("+%1 more").arg(c.apps.size() - shown));
        }
        y = c.apps.isEmpty() ? y : cy + 42;
        // Folders and documents, three at most.
        const qreal bottomLimit = r.bottom() - 74;
        p.setFont(glass::font(13.5));
        for (int k = 0; k < c.places.size() && k < 3 && y + 22 <= bottomLimit; ++k) {
            const QString path = c.places[k].first;
            const QString base = QFileInfo(path).fileName().isEmpty() ? path : QFileInfo(path).fileName();
            p.setPen(QPen(th.ink2, 1.3)); glass::glyph(p, c.places[k].second ? "folder" : "file", QPointF(x + 9, y + 11), .85);
            p.setPen(th.ink);
            const QFontMetricsF pm(p.font());
            const qreal bw = qMin(pm.horizontalAdvance(base) + 2, w * .5);
            p.drawText(QRectF(x + 26, y, bw, 22), Qt::AlignVCenter | Qt::AlignLeft, pm.elidedText(base, Qt::ElideRight, bw));
            p.setPen(th.faint);
            const QString dir = QDir::toNativeSeparators(QFileInfo(path).path());
            p.drawText(QRectF(x + 34 + bw, y, w - bw - 34, 22), Qt::AlignVCenter | Qt::AlignLeft,
                pm.elidedText(dir, Qt::ElideMiddle, w - bw - 34));
            y += 26;
        }
        // Actions and date.
        const auto [resume, more] = activeButtons();
        paintCapsule(p, th, resume, L("Resume this space"), "resume", true, hoverButton == 1, pressedResume);
        paintCapsule(p, th, more, L("More"), "more", false, hoverButton == 2);
        p.setPen(th.faint); p.setFont(glass::font(12));
        const QString date = c.modified != c.created ? L("Updated %1").arg(shortDate(c.modified)) : L("Captured %1").arg(shortDate(c.created));
        p.drawText(QRectF(resume.right() + 12, resume.top(), more.left() - resume.right() - 24, resume.height()),
            Qt::AlignVCenter | Qt::AlignRight, date);
        p.restore();
    }

    // --- input ------------------------------------------------------------

    int buttonAt(QPointF pos) const {
        if (cards.isEmpty()) return emptyCaptureButton().contains(pos) ? 3 : 0;
        const auto [resume, more] = activeButtons();
        if (resume.contains(pos)) return 1;
        if (more.contains(pos)) return 2;
        return 0;
    }

    void mouseMoveEvent(QMouseEvent *e) override {
        const QPointF pos = e->position();
        if (dragStart.x() >= 0 && (e->buttons() & Qt::LeftButton) && !cards.isEmpty()) {
            const qreal dy = pos.y() - dragStart.y();
            if (std::abs(dy) > 24) select(dragFrom - int(dy / band));
            return;
        }
        const int b = buttonAt(pos);
        if (b != hoverButton) { hoverButton = b; update(); }
        const int over = cardAt(pos);
        setCursor(b || (over >= 0 && over != current) ? Qt::PointingHandCursor : Qt::ArrowCursor);
    }
    void leaveEvent(QEvent *) override { hoverButton = 0; update(); }

    void mousePressEvent(QMouseEvent *e) override {
        if (keyboard) { keyboard = false; update(); }
        if (e->button() != Qt::LeftButton) return;
        const QPointF pos = e->position();
        const int b = buttonAt(pos);
        if (b == 1) { pressedResume = true; update(); return; }
        if (b) return;
        const int i = cardAt(pos);
        if (i >= 0 && i != current) { select(i); return; }
        dragStart = pos; dragFrom = current;
    }
    void mouseReleaseEvent(QMouseEvent *e) override {
        const QPointF pos = e->position();
        const int b = buttonAt(pos);
        if (pressedResume) {
            pressedResume = false; update();
            if (b == 1 && resumeRequested) resumeRequested(current);
        } else if (b == 2 && menuRequested) {
            menuRequested(current, mapToGlobal(moreButton().bottomLeft().toPoint() + QPoint(0, 6)));
        } else if (b == 3 && captureRequested) {
            captureRequested();
        }
        dragStart = QPointF(-1, -1);
    }
    void mouseDoubleClickEvent(QMouseEvent *e) override {
        const int i = cardAt(e->position());
        if (i == current && !buttonAt(e->position()) && resumeRequested) resumeRequested(current);
    }
    void wheelEvent(QWheelEvent *e) override {
        wheelAccumulated += e->angleDelta().y();
        while (wheelAccumulated >= 120) { select(current - 1); wheelAccumulated -= 120; }
        while (wheelAccumulated <= -120) { select(current + 1); wheelAccumulated += 120; }
        e->accept();
    }
    void contextMenuEvent(QContextMenuEvent *e) override {
        if (cards.isEmpty()) return;
        const int i = e->reason() == QContextMenuEvent::Mouse ? cardAt(e->pos()) : current;
        if (i < 0) return;
        select(i);
        const QPoint at = e->reason() == QContextMenuEvent::Mouse ? e->globalPos() : mapToGlobal(moreButton().bottomLeft().toPoint());
        if (menuRequested) menuRequested(current, at);
    }
    void keyPressEvent(QKeyEvent *e) override {
        if (!keyboard) { keyboard = true; update(); }
        if (cards.isEmpty()) {
            if ((e->key() == Qt::Key_Return || e->key() == Qt::Key_Enter || e->key() == Qt::Key_Space) && captureRequested) captureRequested();
            else QWidget::keyPressEvent(e);
            return;
        }
        switch (e->key()) {
        case Qt::Key_Up: case Qt::Key_Left: select(current - 1); break;
        case Qt::Key_Down: case Qt::Key_Right: select(current + 1); break;
        case Qt::Key_PageUp: select(current - 3); break;
        case Qt::Key_PageDown: select(current + 3); break;
        case Qt::Key_Home: select(0); break;
        case Qt::Key_End: select(int(cards.size()) - 1); break;
        case Qt::Key_Return: case Qt::Key_Enter: case Qt::Key_Space: if (resumeRequested) resumeRequested(current); break;
        case Qt::Key_Delete: if (deleteRequested) deleteRequested(current); break;
        case Qt::Key_F2: if (editRequested) editRequested(current); break;
        case Qt::Key_Menu: if (menuRequested) menuRequested(current, mapToGlobal(moreButton().bottomLeft().toPoint())); break;
        default: QWidget::keyPressEvent(e);
        }
    }
    void focusInEvent(QFocusEvent *e) override {
        if (e->reason() == Qt::TabFocusReason || e->reason() == Qt::BacktabFocusReason) keyboard = true;
        update();
    }
    void focusOutEvent(QFocusEvent *) override { update(); }
};
