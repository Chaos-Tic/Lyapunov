#pragma once
#include <QtWidgets>
#include <cmath>
#include "i18n.h"

// The Liquid Glass look: a soft peach, blue and lilac environment, with
// translucent surfaces on top (a tinted body, a bright rim, an inner sheen
// and a diffuse shadow). The environment is painted by the window itself, so
// the glass looks the same on every desktop, with or without compositor blur.
// "Reduce transparency" swaps every surface for an opaque one.
namespace glass {

inline QColor rgba(int r, int g, int b, qreal a) { return QColor::fromRgbF(r / 255.0, g / 255.0, b / 255.0, a); }
inline QColor alpha(QColor c, qreal a) { c.setAlphaF(qBound(0.0, a, 1.0)); return c; }
inline QColor mix(const QColor &a, const QColor &b, qreal t) {
    return QColor::fromRgbF(a.redF() + (b.redF() - a.redF()) * t, a.greenF() + (b.greenF() - a.greenF()) * t,
        a.blueF() + (b.blueF() - a.blueF()) * t, a.alphaF() + (b.alphaF() - a.alphaF()) * t);
}

inline bool darkScheme() {
    // LYAPUNOV_THEME=light|dark forces the look (documentation captures).
    if (const QString forced = qEnvironmentVariable("LYAPUNOV_THEME"); forced == "dark" || forced == "light") return forced == "dark";
#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
    if (const auto scheme = QGuiApplication::styleHints()->colorScheme(); scheme != Qt::ColorScheme::Unknown)
        return scheme == Qt::ColorScheme::Dark;
#endif
    return QGuiApplication::palette().color(QPalette::Window).lightness() < 128;
}

// The system's "reduce motion" preference where it can be read, or ours.
inline bool reducedMotion() {
    if (qEnvironmentVariableIsSet("LYAPUNOV_REDUCED_MOTION")) return qEnvironmentVariable("LYAPUNOV_REDUCED_MOTION") != "0";
    if (lyapunov::settings().value("ReduceMotion", false).toBool()) return true;
    static const bool system = [] {
#ifdef Q_OS_WIN
        // SPI_GETCLIENTAREAANIMATION is read by the desktop main through Qt's style hints.
        return false;
#else
        const QString kde = QStandardPaths::locate(QStandardPaths::GenericConfigLocation, "kdeglobals");
        if (!kde.isEmpty() && QSettings(kde, QSettings::IniFormat).value("KDE/AnimationDurationFactor", 1.0).toDouble() == 0.0) return true;
        if (qEnvironmentVariable("XDG_CURRENT_DESKTOP").contains("GNOME", Qt::CaseInsensitive)) {
            QProcess p; p.start("gsettings", {"get", "org.gnome.desktop.interface", "enable-animations"});
            if (p.waitForFinished(800) && p.readAllStandardOutput().trimmed() == "false") return true;
        }
        return false;
#endif
    }();
    return system;
}

inline bool opaque() {
    if (qEnvironmentVariableIsSet("LYAPUNOV_REDUCED_TRANSPARENCY")) return qEnvironmentVariable("LYAPUNOV_REDUCED_TRANSPARENCY") != "0";
    return lyapunov::settings().value("ReduceTransparency", false).toBool();
}

struct Theme {
    bool dark;
    QColor ink, ink2, faint, glass, glassStrong, rim, rimLow, shadow, surface, scrim, focus;
};

inline Theme theme() {
    if (darkScheme())
        return {true, QColor("#f3f4f8"), QColor("#bcc2d0"), QColor("#8a90a2"), rgba(30, 30, 52, .44), rgba(36, 36, 60, .72),
            rgba(255, 255, 255, .34), rgba(255, 255, 255, .08), rgba(0, 0, 0, .55), QColor("#23253a"), rgba(8, 8, 18, .55),
            QColor("#9fdcf8")};
    return {false, QColor("#151820"), QColor("#4a5060"), QColor("#737a8c"), rgba(255, 255, 255, .34), rgba(255, 255, 255, .62),
        rgba(255, 255, 255, .9), rgba(255, 255, 255, .35), rgba(42, 30, 92, .26), QColor("#f4f2f8"), rgba(30, 24, 60, .22),
        QColor("#2b6fd6")};
}

// A card's colour. Names are shown next to swatches: colour is never the only
// way to tell cards apart.
struct Tint {
    QString id, label;
    QColor base, lite, deep;
};

inline const QVector<Tint> &tints() {
    static const QVector<Tint> all{
        {"peach", "Peach", QColor("#ffad85"), QColor("#ffd6c0"), QColor("#b8532a")},
        {"sky", "Sky", QColor("#5fbcf2"), QColor("#bfe6fb"), QColor("#1f6f9e")},
        {"lilac", "Lilac", QColor("#ad8cf0"), QColor("#ddd0fa"), QColor("#5d3fa8")},
        {"mint", "Mint", QColor("#5fd0aa"), QColor("#c4f0e0"), QColor("#1f7a5a")},
        {"amber", "Amber", QColor("#ffbf52"), QColor("#ffe5b5"), QColor("#9a5d00")},
        {"rose", "Rose", QColor("#f48fb4"), QColor("#fcd2e1"), QColor("#a8335d")},
        {"slate", "Slate", QColor("#8e9bb8"), QColor("#d6dbe6"), QColor("#3c4760")},
    };
    return all;
}

inline const Tint &tint(const QString &id) {
    for (const auto &t : tints()) if (t.id == id) return t;
    return tints().first();
}

inline QFont font(qreal pixels, QFont::Weight weight = QFont::Normal) {
    QFont f = QApplication::font();
    f.setFamilies({"Geist", "Segoe UI Variable Text", "Segoe UI", "Inter", "Cantarell", "Noto Sans", f.family()});
    f.setPixelSize(qRound(pixels)); f.setWeight(weight);
    return f;
}

// The environment behind the glass: three soft colour fields on a pale (or
// deep) base. Painted once per size.
inline QPixmap environment(QSize size, qreal dpr, const Theme &th) {
    QPixmap pm(size * dpr); pm.setDevicePixelRatio(dpr);
    QPainter p(&pm); p.setRenderHint(QPainter::Antialiasing);
    const QRectF r(QPointF(0, 0), QSizeF(size));
    QLinearGradient base(r.topLeft(), r.bottomRight());
    if (th.dark) { base.setColorAt(0, QColor("#17182a")); base.setColorAt(.5, QColor("#141a2a")); base.setColorAt(1, QColor("#1d1630")); }
    else { base.setColorAt(0, QColor("#fbefe8")); base.setColorAt(.5, QColor("#eef0fb")); base.setColorAt(1, QColor("#f1e8fa")); }
    p.fillRect(r, base);
    auto blob = [&](QPointF c, qreal radius, QColor color, qreal a) {
        QRadialGradient g(c, radius);
        g.setColorAt(0, alpha(color, a)); g.setColorAt(.55, alpha(color, a * .45)); g.setColorAt(1, alpha(color, 0));
        p.fillRect(r, g);
    };
    const qreal s = qMax(r.width(), r.height());
    const qreal k = th.dark ? .38 : .75;
    blob({r.width() * .08, r.height() * .1}, s * .55, QColor("#ffb48c"), k);
    blob({r.width() * .95, r.height() * .3}, s * .5, QColor("#8fd0f6"), k);
    blob({r.width() * .45, r.height() * 1.02}, s * .55, QColor("#c3a6f5"), k);
    return pm;
}

// A soft shadow under a rounded shape, built from expanding translucent rings.
inline void shadow(QPainter &p, const QRectF &r, qreal radius, const QColor &color, qreal strength, qreal lift = 12) {
    if (strength <= 0) return;
    p.save(); p.setPen(Qt::NoPen);
    const int layers = 9;
    for (int i = layers; i >= 1; --i) {
        const qreal grow = i * 2.6;
        p.setBrush(alpha(color, color.alphaF() * strength * 0.16 * (1.0 - qreal(i - 1) / layers)));
        p.drawRoundedRect(r.adjusted(-grow, -grow + lift * .6, grow, grow + lift), radius + grow, radius + grow);
    }
    p.restore();
}

// A glass surface. `presence` (0..1) brightens and thickens it: the active
// card, a pressed button.
inline void surface(QPainter &p, const QRectF &r, qreal radius, const Theme &th, const QColor &tintColor, qreal presence,
    qreal tintAmount = 0.0) {
    QPainterPath path; path.addRoundedRect(r, radius, radius);
    p.save(); p.setPen(Qt::NoPen);
    if (opaque()) {
        p.setBrush(mix(th.surface, tintColor, tintAmount * (th.dark ? .35 : .45)));
        p.drawPath(path);
    } else {
        QLinearGradient body(r.topLeft(), r.bottomRight());
        const qreal a = th.dark ? .34 + .22 * presence : .30 + .22 * presence;
        body.setColorAt(0, alpha(mix(th.glassStrong, tintColor, tintAmount * .55), a + .14));
        body.setColorAt(1, alpha(mix(th.glass, tintColor, tintAmount * .75), a));
        p.setBrush(body); p.drawPath(path);
        // Inner sheen over the upper part.
        QLinearGradient sheen(r.topLeft(), QPointF(r.left(), r.top() + qMin(r.height(), 140.0)));
        sheen.setColorAt(0, rgba(255, 255, 255, (th.dark ? .16 : .42) * (.6 + .4 * presence)));
        sheen.setColorAt(1, rgba(255, 255, 255, 0));
        p.setBrush(sheen); p.drawPath(path);
    }
    // Bright rim above, quieter below.
    QLinearGradient rim(r.topLeft(), r.bottomLeft());
    rim.setColorAt(0, alpha(th.rim, th.rim.alphaF() * (.75 + .25 * presence)));
    rim.setColorAt(1, th.rimLow);
    p.setBrush(Qt::NoBrush); p.setPen(QPen(QBrush(rim), 1.2));
    p.drawRoundedRect(r.adjusted(.6, .6, -.6, -.6), radius - .6, radius - .6);
    p.restore();
}

// The card emblem: a disc in the card's colour with the trajectory that
// settles on a point, Lyapunov's sign of a state you return to.
inline void emblem(QPainter &p, QPointF c, qreal radius, const Tint &t, const Theme &th) {
    p.save();
    QLinearGradient g(c - QPointF(radius, radius), c + QPointF(radius, radius));
    g.setColorAt(0, t.lite); g.setColorAt(1, t.base);
    p.setPen(QPen(alpha(Qt::white, th.dark ? .35 : .8), 1)); p.setBrush(g);
    p.drawEllipse(c, radius, radius);
    QPainterPath spiral;
    const int steps = 60; const qreal turns = 1.8, k = .22;
    for (int i = 0; i <= steps; ++i) {
        const qreal t2 = i / qreal(steps) * turns * 2 * M_PI;
        const qreal rr = radius * .62 * std::exp(-k * t2);
        const QPointF pt = c + QPointF(rr * std::cos(3.5 - t2), rr * std::sin(3.5 - t2));
        if (i == 0) spiral.moveTo(pt); else spiral.lineTo(pt);
    }
    p.setBrush(Qt::NoBrush); p.setPen(QPen(Qt::white, qMax(1.4, radius * .16), Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    p.drawPath(spiral);
    p.setPen(Qt::NoPen); p.setBrush(Qt::white); p.drawEllipse(c, radius * .14, radius * .14);
    p.restore();
}

// Small line glyphs, drawn in the current pen.
inline void glyph(QPainter &p, const QString &name, QPointF c, qreal s = 1.0) {
    p.save(); p.translate(c); p.scale(s, s); p.setBrush(Qt::NoBrush);
    if (name == "folder") {
        QPainterPath f; f.moveTo(-8, -5); f.lineTo(-3, -5); f.lineTo(-1.5, -3); f.lineTo(8, -3); f.lineTo(8, 6); f.lineTo(-8, 6); f.closeSubpath(); p.drawPath(f);
    } else if (name == "file") {
        QPainterPath d; d.moveTo(-5.5, -7); d.lineTo(2, -7); d.lineTo(5.5, -3.5); d.lineTo(5.5, 7); d.lineTo(-5.5, 7); d.closeSubpath(); p.drawPath(d);
        p.drawLine(QPointF(-2.5, 0), QPointF(2.5, 0)); p.drawLine(QPointF(-2.5, 3), QPointF(2.5, 3));
    } else if (name == "app") {
        p.drawRoundedRect(QRectF(-7.5, -6, 15, 12), 3, 3); p.drawLine(QPointF(-7.5, -2.5), QPointF(7.5, -2.5));
    } else if (name == "check") {
        p.drawLine(QPointF(-5, 0), QPointF(-1.5, 4)); p.drawLine(QPointF(-1.5, 4), QPointF(6, -4.5));
    } else if (name == "start") {
        QPainterPath t; t.moveTo(-4, -6); t.lineTo(6, 0); t.lineTo(-4, 6); t.closeSubpath(); p.drawPath(t);
    } else if (name == "missing") {
        p.drawEllipse(QPointF(0, 0), 6.5, 6.5); p.drawLine(QPointF(-4.5, 4.5), QPointF(4.5, -4.5));
    } else if (name == "fail") {
        p.drawLine(QPointF(-4.5, -4.5), QPointF(4.5, 4.5)); p.drawLine(QPointF(-4.5, 4.5), QPointF(4.5, -4.5));
    } else if (name == "skip") {
        p.drawLine(QPointF(-6, 0), QPointF(6, 0));
    } else if (name == "more") {
        for (int i : {-6, 0, 6}) { p.save(); p.setBrush(p.pen().color()); p.setPen(Qt::NoPen); p.drawEllipse(QPointF(i, 0), 1.7, 1.7); p.restore(); }
    } else if (name == "plus") {
        p.drawLine(QPointF(-6, 0), QPointF(6, 0)); p.drawLine(QPointF(0, -6), QPointF(0, 6));
    } else if (name == "resume") {
        p.drawArc(QRectF(-6.5, -6.5, 13, 13), 60 * 16, 290 * 16);
        p.drawLine(QPointF(3.2, -5.6), QPointF(6.4, -6.4)); p.drawLine(QPointF(3.2, -5.6), QPointF(3.9, -2.4));
    }
    p.restore();
}

// Theme-aware style for the real widgets inside panels: capsules, fields,
// check boxes, scroll areas. Painted glass behind them stays visible.
inline QString styleSheet(const Theme &th) {
    const QString ink = th.ink.name(), ink2 = th.ink2.name();
    const QString field = th.dark ? "rgba(255,255,255,0.08)" : "rgba(255,255,255,0.62)";
    const QString fieldLine = th.dark ? "rgba(255,255,255,0.18)" : "rgba(21,24,32,0.14)";
    const QString button = th.dark ? "rgba(255,255,255,0.10)" : "rgba(255,255,255,0.58)";
    const QString buttonHover = th.dark ? "rgba(255,255,255,0.18)" : "rgba(255,255,255,0.86)";
    const QString primary = th.dark ? "#9fdcf8" : "#151820";
    const QString primaryInk = th.dark ? "#10131c" : "#ffffff";
    const QString focus = th.focus.name();
    return QString(R"(
        QWidget { color: %1; }
        QLabel#muted, QLabel#hint { color: %2; }
        QLabel#title { font-weight: 600; }
        QLabel#section { color: %2; font-weight: 600; letter-spacing: 0.4px; }
        QLineEdit, QPlainTextEdit { background: %3; border: 1px solid %4; border-radius: 12px; padding: 7px 11px; selection-background-color: %9; }
        QLineEdit:focus, QPlainTextEdit:focus { border: 2px solid %9; padding: 6px 10px; }
        QPushButton { background: %5; border: 1px solid %4; border-radius: 17px; padding: 8px 18px; min-height: 18px; }
        QPushButton:hover { background: %6; }
        QPushButton:focus { border: 2px solid %9; padding: 7px 17px; }
        QPushButton:disabled { color: %2; }
        QPushButton#primary { background: %7; color: %8; border: none; font-weight: 600; padding: 9px 22px; }
        QPushButton#primary:focus { border: 2px solid %9; padding: 7px 20px; }
        QPushButton#primary:disabled { background: %4; color: %2; }
        QPushButton#flat { background: transparent; border: none; padding: 4px 8px; color: %2; }
        QPushButton#flat:hover { color: %1; }
        QPushButton#flat:focus { border: 2px solid %9; border-radius: 8px; }
        QToolButton#swatch { border: 2px solid transparent; border-radius: 15px; min-width: 26px; min-height: 26px; }
        QToolButton#swatch:checked { border: 2px solid %1; }
        QToolButton#swatch:focus { border: 2px solid %9; }
        QCheckBox { spacing: 10px; }
        QCheckBox:focus { color: %9; }
        QScrollArea, QScrollArea > QWidget > QWidget { background: transparent; border: none; }
        QScrollBar:vertical { background: transparent; width: 8px; margin: 2px; }
        QScrollBar::handle:vertical { background: %4; border-radius: 3px; min-height: 30px; }
        QScrollBar::add-line, QScrollBar::sub-line { height: 0; }
        QMenu { background: %10; border: 1px solid %4; border-radius: 10px; padding: 6px; }
        QMenu::item { padding: 7px 22px 7px 14px; border-radius: 6px; }
        QMenu::item:selected { background: %5; }
        QMenu::separator { height: 1px; background: %4; margin: 5px 8px; }
        QToolTip { color: %1; background: %10; border: 1px solid %4; }
    )").arg(ink, ink2, field, fieldLine, button, buttonHover, primary, primaryInk, focus, th.surface.name());
}

// A damped spring; retargeting keeps the velocity, so motion never jumps.
struct Spring {
    qreal x = 0, v = 0, target = 0;
    void snap(qreal value) { x = target = value; v = 0; }
    bool step(qreal dt, qreal k = 340, qreal c = 34) {
        const int n = qMax(1, int(std::ceil(dt / 0.004)));
        const qreal h = dt / n;
        for (int i = 0; i < n; ++i) { v += (-k * (x - target) - c * v) * h; x += v * h; }
        if (std::abs(x - target) < 0.05 && std::abs(v) < 0.05) { snap(target); return false; }
        return true;
    }
};

} // namespace glass
