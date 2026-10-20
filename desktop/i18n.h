#pragma once
#include <QtCore>

// Interface language and preferences. lyapunov.conf holds "Language" ("fr",
// "en" or "auto", the default, which follows the system), "ReduceMotion" and
// "ReduceTransparency". Text is written in English in the code; L() returns
// the French text when the interface is French.
namespace lyapunov {
inline QString configFile() {
    return QStandardPaths::writableLocation(QStandardPaths::GenericConfigLocation) + "/lyapunov/lyapunov.conf";
}
inline QSettings &settings() {
    static QSettings s(configFile(), QSettings::IniFormat);
    return s;
}
inline bool french() {
    static const bool value = [] {
        const QString forced = qEnvironmentVariable("LYAPUNOV_LANGUAGE");
        const QString chosen = forced.isEmpty() ? settings().value("Language", "auto").toString() : forced;
        if (chosen == "fr" || chosen == "en") return chosen == "fr";
        return QLocale::system().language() == QLocale::French;
    }();
    return value;
}
inline QString L(const QString &english) {
    if (!french()) return english;
    static const QHash<QString, QString> fr{
        // Window and stack
        {"Your spaces", "Vos espaces"}, {"Capture this space", "Capturer cet espace"},
        {"Resume this space", "Reprendre cet espace"}, {"More", "Plus"}, {"Menu", "Menu"},
        {"No cards yet", "Aucune carte pour l'instant"},
        {"A card keeps the applications, windows, folders and documents of one activity. Capture what is open now, and come back to it later.",
         "Une carte garde les applications, fenêtres, dossiers et documents d'une activité. Capturez ce qui est ouvert maintenant, et revenez-y plus tard."},
        {"%1 apps", "%1 applis"}, {"1 app", "1 appli"}, {"%1 windows", "%1 fenêtres"}, {"1 window", "1 fenêtre"},
        {"%1 places", "%1 emplacements"}, {"1 place", "1 emplacement"}, {"Nothing saved yet", "Rien d'enregistré"},
        {"Captured %1", "Capturée le %1"}, {"Updated %1", "Mise à jour le %1"},
        {"Card %1 of %2", "Carte %1 sur %2"}, {"selected", "sélectionnée"},
        {"Press Enter to resume, the menu key for more.", "Entrée pour reprendre, touche menu pour plus d'options."},
        {"Cards", "Cartes"}, {"+%1 more", "+%1 autres"},
        // Menus
        {"Edit…", "Modifier…"}, {"Update from what is open…", "Mettre à jour depuis ce qui est ouvert…"},
        {"Duplicate", "Dupliquer"}, {"Move up", "Monter"}, {"Move down", "Descendre"}, {"Export…", "Exporter…"},
        {"Delete…", "Supprimer…"}, {"Import cards…", "Importer des cartes…"}, {"Export all cards…", "Exporter toutes les cartes…"},
        {"Open the cards folder", "Ouvrir le dossier des cartes"}, {"Language", "Langue"}, {"Automatic", "Automatique"},
        {"Reduce motion", "Réduire les animations"}, {"Reduce transparency", "Réduire la transparence"},
        {"About Lyapunov", "À propos de Lyapunov"}, {"Diagnostics", "Diagnostic"},
        {"The language changes the next time Lyapunov opens.", "La langue change à la prochaine ouverture de Lyapunov."},
        {"Set up GNOME integration", "Activer l'intégration GNOME"},
        // Capture
        {"Name", "Nom"}, {"Name this activity", "Nommez cette activité"}, {"Colour", "Couleur"},
        {"Applications and windows", "Applications et fenêtres"}, {"Folders and documents", "Dossiers et documents"},
        {"Add a folder…", "Ajouter un dossier…"}, {"Add a document…", "Ajouter un document…"},
        {"Drop folders or documents here", "Déposez des dossiers ou documents ici"},
        {"Save card", "Enregistrer la carte"}, {"Cancel", "Annuler"}, {"Remove", "Retirer"},
        {"Lyapunov cannot restart it; its windows are arranged when it is open.",
         "Lyapunov ne peut pas la relancer ; ses fenêtres sont replacées quand elle est ouverte."},
        {"Nothing is moved or closed while capturing.", "Rien n'est déplacé ni fermé pendant la capture."},
        {"Window titles are shown here to help you choose. They are not saved.",
         "Les titres des fenêtres s'affichent ici pour vous aider à choisir. Ils ne sont pas enregistrés."},
        {"No application window is open.", "Aucune fenêtre d'application n'est ouverte."},
        {"Give the card a name.", "Donnez un nom à la carte."},
        {"Peach", "Pêche"}, {"Sky", "Ciel"}, {"Lilac", "Lilas"}, {"Mint", "Menthe"}, {"Amber", "Ambre"}, {"Rose", "Rose"},
        {"Slate", "Ardoise"}, {"Reading the windows…", "Lecture des fenêtres…"},
        {"Update “%1”", "Mettre à jour « %1 »"}, {"Replace the card", "Remplacer la carte"},
        {"Added", "Ajoutées"}, {"No longer open", "Plus ouvertes"}, {"Moved", "Déplacées"},
        {"Nothing changed since the card was saved.", "Rien n'a changé depuis l'enregistrement de la carte."},
        // Edit
        {"Edit “%1”", "Modifier « %1 »"}, {"Note", "Note"}, {"A few words about this activity (optional)", "Quelques mots sur cette activité (facultatif)"},
        {"Show what will happen before resuming", "Montrer ce qui va se passer avant de reprendre"},
        {"Move windows that are already open", "Déplacer les fenêtres déjà ouvertes"}, {"Save", "Enregistrer"},
        // Resume
        {"Resume “%1”", "Reprendre « %1 »"}, {"Resume", "Reprendre"}, {"Will be started", "Sera lancée"},
        {"Open — windows arranged", "Ouverte — fenêtres replacées"}, {"Open", "Ouverte"}, {"Will be opened", "Sera ouvert"},
        {"Unavailable", "Indisponible"}, {"not installed", "non installée"}, {"not found", "introuvable"},
        {"runs in a terminal", "s'exécute dans un terminal"},
        {"Lyapunov cannot start this application (no menu entry)", "Lyapunov ne peut pas lancer cette application (pas d'entrée de menu)"},
        {"Also move windows that are already open", "Déplacer aussi les fenêtres déjà ouvertes"},
        {"Resuming…", "Reprise…"}, {"Starting %1…", "Lancement de %1…"}, {"Opening %1…", "Ouverture de %1…"},
        {"Waiting for windows…", "Attente des fenêtres…"}, {"Arranging %1 windows…", "Placement de %1 fenêtres…"},
        {"Stop waiting", "Ne plus attendre"},
        {"Stopping here leaves what has already started open.", "S'arrêter ici laisse ouvert ce qui a déjà démarré."},
        {"Nothing selected.", "Rien n'est sélectionné."},
        // Report
        {"“%1” resumed", "« %1 » reprise"}, {"Done", "Terminé"}, {"Window found", "Fenêtre retrouvée"},
        {"Application started", "Application relancée"}, {"Already open", "Déjà ouverte"}, {"Opened", "Ouvert"},
        {"Failed", "Échec"}, {"Left out", "Laissé de côté"}, {"%1 of %2 windows placed", "%1 fenêtre(s) sur %2 replacée(s)"},
        {"Close Lyapunov", "Fermer Lyapunov"}, {"Back to cards", "Retour aux cartes"},
        {"started, but no window appeared in time to be arranged", "lancée, mais aucune fenêtre n'est apparue à temps pour être replacée"},
        {"windows are not arranged on this desktop", "les fenêtres ne sont pas replacées sur ce bureau"},
        {"a window could not be moved", "une fenêtre n'a pas pu être déplacée"},
        {"window closed", "fenêtre fermée"}, {"this window cannot be moved", "cette fenêtre ne peut pas être déplacée"},
        // Delete and problems
        {"Delete “%1”?", "Supprimer « %1 » ?"},
        {"Only the card is deleted. The applications, folders and documents it lists are not touched.",
         "Seule la carte est supprimée. Les applications, dossiers et documents qu'elle cite ne sont pas touchés."},
        {"Delete", "Supprimer"}, {"%1 card(s) could not be read", "%1 carte(s) illisible(s)"}, {"Details", "Détails"},
        {"Unreadable cards", "Cartes illisibles"},
        {"These files were left untouched. Fix or remove them in the cards folder.",
         "Ces fichiers n'ont pas été modifiés. Corrigez-les ou retirez-les dans le dossier des cartes."},
        {"Close", "Fermer"},
        // Limitations and errors
        {"Windows are not arranged on this desktop: applications, folders and documents still reopen.",
         "Les fenêtres ne sont pas replacées sur ce bureau : applications, dossiers et documents se rouvrent quand même."},
        {"On GNOME, arranging windows needs the Lyapunov extension.", "Sur GNOME, replacer les fenêtres demande l'extension Lyapunov."},
        {"Log out and back in to finish setting up GNOME integration.",
         "Déconnectez-vous puis reconnectez-vous pour terminer l'intégration GNOME."},
        {"The Lyapunov core (lyapunov) was not found. Reinstall Lyapunov.",
         "Le cœur de Lyapunov (lyapunov) est introuvable. Réinstallez Lyapunov."},
        {"Something went wrong", "Un problème est survenu"},
        {"Import “%1”", "Importer « %1 »"}, {"Import", "Importer"}, {"Starts", "Lance"}, {"Opens", "Ouvre"},
        {"Rejected", "Refusée"},
        {"Imported cards never start anything until you resume them.",
         "Les cartes importées ne lancent rien tant que vous ne les reprenez pas."},
        {"Lyapunov cards (*.json)", "Cartes Lyapunov (*.json)"}, {"Exported to %1", "Exporté vers %1"},
        {"Save a card and pick it up later.", "Enregistrez une activité, reprenez-la plus tard."},
        {"Add Lyapunov to the applications menu?", "Ajouter Lyapunov au menu des applications ?"}, {"Add", "Ajouter"},
        {"Version %1. Everything stays on this computer: no account, no network.",
         "Version %1. Tout reste sur cet ordinateur : aucun compte, aucun réseau."},
    };
    return fr.value(english, english);
}
} // namespace lyapunov
using lyapunov::L;
