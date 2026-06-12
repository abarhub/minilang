# Configuration de projet — `minilang.toml`

Un projet minilang peut être configuré par un fichier **`minilang.toml`**, placé à la racine du projet.

Le fichier est **optionnel** : sans lui, l'interpréteur fonctionne exactement comme avant (fichier source en argument, tous les modules DI actifs, log `info`).

## Découverte

Le fichier est cherché dans le répertoire du fichier source passé en argument (ou le répertoire courant si aucun argument), puis en **remontant les répertoires parents**. Le premier trouvé est utilisé.

Priorité des réglages : **arguments CLI > `minilang.toml` > défauts**.

Un fichier présent mais invalide (TOML cassé, section ou clé inconnue — typo) est une **erreur fatale** : une configuration cassée ne doit pas être silencieusement ignorée.

## Format

```toml
[project]
name = "mon-appli"          # affiché dans les logs
main = "src/app.mini"       # point d'entrée, relatif au minilang.toml

[di]
modules = ["ProdModule"]    # modules de binding actifs

[runtime]
log = "info"                # niveau de log par défaut
```

Toutes les sections et toutes les clés sont optionnelles.

### `[project]`

| Clé | Effet | Défaut |
|---|---|---|
| `name` | Nom du projet, affiché dans les logs | — |
| `main` | Point d'entrée, relatif au répertoire du `minilang.toml`. Permet de lancer `mini_parser` **sans argument** depuis le projet | — (argument CLI obligatoire) |

### `[di]` — profils d'injection de dépendances

| Clé | Effet | Défaut |
|---|---|---|
| `modules` | Liste des modules de binding **actifs**. Les autres modules du programme sont ignorés. Une liste vide désactive tous les modules. Un nom inconnu est une erreur | absent = tous les modules actifs |

C'est le mécanisme de **profils** : le code déclare plusieurs modules (prod, test, …) et la configuration choisit, sans toucher au code :

```java
module ProdModule { bind Repo to SqlRepo; }
module TestModule { bind Repo to FakeRepo; }
```

```toml
[di]
modules = ["ProdModule"]    # ou ["TestModule"] pour les mocks
```

Sans sélection, tous les modules sont fusionnés — deux `bind` pour la même interface seraient alors un binding dupliqué (erreur de compilation).

### `[runtime]`

| Clé | Effet | Défaut |
|---|---|---|
| `log` | Niveau de log (`error`, `warn`, `info`, `debug`, `trace`). La variable d'environnement `RUST_LOG` reste prioritaire | `info` |

## Exemple complet

Voir [examples/example_config/](../examples/example_config/) : un projet avec deux profils DI, lançable avec `mini_parser examples/example_config/app.mini` ou sans argument depuis son répertoire.
