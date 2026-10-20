// The card stack offscreen: layout, hit testing, keyboard and actions.
// LYAPUNOV_STACK_FRAMES=DIR also writes frames for visual review.
#include <QtWidgets>
#include <QTest>
#include "../desktop/cardstack.h"

static int failures = 0;
static void check(bool ok, const char *what) {
    if (!ok) { ++failures; qWarning("FAIL: %s", what); } else qInfo("ok  %s", what);
}

static QVector<CardView> demo(int n) {
    const QStringList names{"Site personnel", "Photos du voyage", "Administratif", "À lire", "Musique", "Thèse"};
    QVector<CardView> list;
    for (int i = 0; i < n; ++i) {
        CardView c; c.id = QString("id-%1").arg(i); c.name = names[i % names.size()];
        c.tint = glass::tints()[i % glass::tints().size()].id; c.apps = {"Firefox", "Dolphin", "Kate"}; c.windows = 4;
        c.places = {{"/home/user/site", true}, {"/home/user/notes.md", false}};
        c.created = c.modified = "2026-10-20T15:04:05Z";
        list << c;
    }
    return list;
}

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    CardStack stack; stack.resize(720, 700); stack.show();
    int resumed = -1, menu = -1, removed = -1;
    stack.resumeRequested = [&](int i) { resumed = i; };
    stack.menuRequested = [&](int i, QPoint) { menu = i; };
    stack.deleteRequested = [&](int i) { removed = i; };
    stack.setCards(demo(5));
    QCoreApplication::processEvents();

    check(stack.currentIndex() == 0, "the first card is active");
    check(stack.cardRect(0).height() > CardStack::band + CardStack::tuck + 60, "the active card is open");
    check(stack.resumeButton().bottom() <= stack.cardRect(0).bottom(), "the Resume button is inside the open card");
    for (int i = 1; i < 5; ++i) {
        const QRectF above = stack.cardRect(i - 1), me = stack.cardRect(i);
        const qreal visible = i - 1 == stack.currentIndex() ? me.top() - above.bottom() : me.top() - above.top();
        check(i - 1 == stack.currentIndex() ? visible > 0 : visible >= CardStack::band - 0.5, "every band stays visible");
    }
    const QPointF bandOf2 = stack.cardRect(2).topLeft() + QPointF(120, 20);
    check(stack.cardAt(bandOf2) == 2, "a band is hit by its own card");
    QTest::mouseClick(&stack, Qt::LeftButton, {}, bandOf2.toPoint());
    QCoreApplication::processEvents();
    check(stack.currentIndex() == 2, "clicking a band selects its card");
    QTest::keyClick(&stack, Qt::Key_Down);
    check(stack.currentIndex() == 3, "the down arrow selects the next card");
    QTest::keyClick(&stack, Qt::Key_End);
    check(stack.currentIndex() == 4, "End selects the last card");
    QTest::keyClick(&stack, Qt::Key_Home);
    check(stack.currentIndex() == 0, "Home selects the first card");
    QTest::keyClick(&stack, Qt::Key_Return);
    check(resumed == 0, "Enter resumes the active card");
    resumed = -1;
    QTest::mouseClick(&stack, Qt::LeftButton, {}, stack.resumeButton().center().toPoint());
    check(resumed == 0, "the Resume button resumes the active card");
    QTest::mouseClick(&stack, Qt::LeftButton, {}, stack.moreButton().center().toPoint());
    check(menu == 0, "the More button opens the menu");
    QTest::keyClick(&stack, Qt::Key_Delete);
    check(removed == 0, "Delete asks to delete the active card");
    check(stack.accessibleName().contains("Site personnel") && stack.accessibleName().contains("1"), "the active card is announced");
    stack.select(4);
    const QRectF last = stack.cardRect(4);
    check(last.bottom() <= stack.height() + 1, "the active card is scrolled into view");
    stack.setCards({});
    check(stack.currentIndex() == -1, "an empty stack has no active card");

    if (const QString dir = qEnvironmentVariable("LYAPUNOV_STACK_FRAMES"); !dir.isEmpty()) {
        QDir().mkpath(dir);
        stack.setCards(demo(5));
        stack.grab().save(dir + "/stack.png");
    }
    return failures ? 1 : 0;
}
