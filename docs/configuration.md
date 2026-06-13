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

[sources]
include = ["lib.mini"]      # code partagé, préfixé au fichier exécuté

[di]
modules = ["ProdModule"]    # modules de binding actifs

[tests]
dir = "tests"               # répertoire des fichiers de tests
modules = ["TestModule"]    # profil DI pendant les tests

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

### `[sources]`

| Clé | Effet | Défaut |
|---|---|---|
| `include` | Fichiers source additionnels (bibliothèque du projet, **sans `main`**), relatifs au `minilang.toml`. Concaténés avant le fichier exécuté — en mode run comme en mode test. C'est ce qui permet aux fichiers de tests de référencer le code de l'application | — |

Limitation actuelle : les fichiers étant concaténés (comme la stdlib), les positions dans les messages d'erreur de syntaxe ne tiennent pas compte du fichier d'origine.

### `[tests]` — runner de tests

| Clé | Effet | Défaut |
|---|---|---|
| `dir` | Répertoire des fichiers de tests (`.mini`, parcouru récursivement), relatif au `minilang.toml`. Utilisé par `mini_parser test` sans argument | `tests` |
| `modules` | Modules de binding actifs **pendant les tests** — le profil de mocks | `[di] modules`, sinon tous |

### `[files.roots]` — racines d'accès fichiers

Octroie au programme des **racines nommées** : des répertoires auxquels il peut accéder via des capacités confinées (voir `docs/io.md`). Le code ne choisit jamais de chemin absolu — il demande une racine par son nom (`FileSystem.root(nom)` / `rootRW(nom)`).

```toml
[files.roots.data]
path = "data"            # relatif au minilang.toml (ou absolu)
mode = "read-write"

[files.roots.assets]
path = "assets"
mode = "read"            # défaut
```

| Clé (par racine) | Effet | Défaut |
|---|---|---|
| `path` | Répertoire de la racine, relatif au `minilang.toml`. **Doit exister au démarrage** (sinon erreur fatale) ; canonicalisé | — (obligatoire) |
| `mode` | `read` (lecture seule) ou `read-write`. `rootRW(nom)` échoue si la racine est `read` | `read` |

Autres clés de `[files]` :

| Clé | Effet | Défaut |
|---|---|---|
| `unrestricted` | Autorise l'accès fichiers **brut** (classe `Files`, chemins arbitraires sans confinement). `false` → seules les capacités confinées (`FileSystem` / racines) sont utilisables ; toute opération `Files` échoue | `false` (sûr par défaut) |
| `temp` | Nettoyage des répertoires de `FileSystem.tempDir()` : `mark` (marqueur `.minilang-temp` à la création, nettoyage externe par âge) / `delete` (suppression en fin de programme, marqueur en filet) / `none` (rien) | `mark` |

### `[runtime]`

| Clé | Effet | Défaut |
|---|---|---|
| `log` | Niveau de log (`error`, `warn`, `info`, `debug`, `trace`). La variable d'environnement `RUST_LOG` reste prioritaire | `info` |

## Exemples complets

- [examples/example_config/](../examples/example_config/) : un projet avec deux profils DI.
- [examples/example_files_config/](../examples/example_files_config/) : racines fichiers nommées (`data` en lecture/écriture, `assets` en lecture seule).
