# A faire

* améliorer le langage
  * mieux gérer les lambda (typage par rapport aux objets, etc...)
  * ajouter un systeme de stream
  * ajouter les tuples
  * gérer les entiers avec une taille différente de 32 bits
  * gérer les entiers non signés
  * mécanisme de pakage et d'import à completer (gestion des classes avec le même nom, arborescence de dossiers, etc...)
  * ajouter le let au niveau des instructions
  * ajouter le let au niveau du if, avec extraction des variables
  * définition des élements incomplet des classes
* completer la librairie standard
  * faire les methodes pour les math, et voir pour les constantes
  * ajouter la classe List, pour avoir des tableaux de taille variable
  * ajouter des conteneurs (Map, Set, etc...)
  * ajouter des classes de concurence (Queue, thread, etc...)
  * modifier la classe String pour que la récupération d'un caractère ou d'un sous-string utilise Option
  * réduire les possibilité de panic dans la lib standard
  * ajouter un mécanisme de reflexion
  * classes pour l'accès au fs
  * classe pour lire l'entrée standard
  * classe pour écrire dans la sortie standard et la sortie en erreur
* global
  * ajouter un mécanisme pur les tests unitaires
  * ajouter un mécanisme de concurence
  * générer de la documentation à partir des classes
  * ajouter un bytecode
  * compiler en code machine
  * regrouper les classes dans des zip

# Reportés (chantiers DI / config / tests / I/O / sûreté fichiers)

Points volontairement laissés de côté au fil des fonctionnalités récentes.

* suites naturelles
  * plugins d'audit sur les capacités fichiers : décorateurs autour d'un `ReadWriteDir` (journalisation des accès, détection de ressources non fermées) — faisable via héritage d'interface + DI
  * appliquer les réglages `[files]` (`roots`, `temp`, `unrestricted`) aussi en mode test (aujourd'hui seulement en mode run ; le runner crée ses propres interpréteurs sans propager ces réglages)
  * streaming de fichiers (`FileInput`/`FileOutput` ligne par ligne) — nécessite une table de handles + `close` + un construct de ressource à portée
  * construct de ressource à portée (`with`/`using`) — préalable propre aux flux de fichiers (pas de destructeurs → risque de fuite de descripteurs)
  * confinement symlink adversarial (niveau OS : `openat`/`RESOLVE_BENEATH`, p. ex. crate `cap-std`) — l'actuel est best-effort coopératif (canonicalisation + containment, TOCTOU résiduel)
* limitations connues
  * positions d'erreur multi-fichiers : `[sources] include` concatène, donc les positions de syntaxe ne tracent pas le fichier d'origine
* choix délibérés, revisitables
  * pas d'arithmétique sur `byte` (ajoutable en enroulement mod 256)
  * `inject` réservé à `main` et aux fonctions top-level (pas dans les méthodes de classe)
  * `print` conservé (peut-être retiré plus tard)
  * pas de `must-use` sur `Result` (un Result ignoré l'est silencieusement) — durcissable
* gros chantiers jamais entamés
  * vraies dépendances externes / système de modules (résolution, versions) dans `minilang.toml` — seul `[sources] include` (concaténation) existe
  * GUI / `[project] type = "console"|"gui"` (écarté tant qu'il n'y a pas de GUI)




