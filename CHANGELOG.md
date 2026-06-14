# Changelog — minilang

Toutes les évolutions notables du langage sont documentées ici.

---

## [14/06/2026] — Généricité de l'héritage de types

Les arguments de type passés à un supertype générique sont désormais **conservés et substitués** le long de la chaîne d'héritage, au lieu d'être ignorés. Cela vaut pour les trois clauses :

- `interface Box<E> extends Container<E>` — propagation entre paramètres (même renommés) ;
- `interface IntSource extends Source<int>` — argument concret sur le parent ;
- `class IntCell extends Base<int>` — méthodes **et** champs hérités d'une classe générique sont substitués (`getVal()` retourne `int`, le champ `val` est un `int`).

```java
interface Container<T> { T get(); }
interface Box<E> extends Container<E> { void put(E x); }
class IntBox implements Box<int> { int get() { ... } void put(int x) {} }

Box<int> bx = new IntBox(7);
int v = bx.get();   // get() héritée de Container, retour résolu en int
```

L'arité des arguments de type est validée (0 argument toléré = paramètres non liés, sinon le nombre doit correspondre). Clôt la limitation « parents d'interface génériques » du backlog.

---

## [13/06/2026] — Accès fichiers brut (`Files`) désactivé par défaut

La classe `Files` (chemins bruts, sans confinement) donne une autorité totale sur le système de fichiers, ce qui contournait le modèle de capacités. Elle est désormais **interdite par défaut** : toute opération `Files` échoue tant que `[files] unrestricted = true` n'est pas explicitement positionné.

```toml
[files]
unrestricted = true     # défaut : false
```

Conséquence : les capacités confinées (`FileSystem` / racines configurées) sont le **mode par défaut** ; l'accès brut devient un opt-in explicite et auditable (échappatoire bas-niveau pour outils de confiance). L'exemple `examples/example_files/` active le drapeau dans son `minilang.toml`.

---

## [13/06/2026] — Nettoyage des répertoires temporaires (`[files] temp`)

Politique configurable pour les répertoires créés par `FileSystem.tempDir()` :

| `[files] temp` | Effet |
|---|---|
| `mark` (défaut) | Pose un marqueur `.minilang-temp` **à la création** ; un nettoyeur externe (cron…) supprime les répertoires marqués selon leur âge. |
| `delete` | Supprime les répertoires temp en fin de `run()` (best-effort) ; le marqueur reste comme filet en cas d'arrêt anormal. |
| `none` | Ne rien faire (pas de marqueur, pas de suppression). |

Le marqueur est posé **dès la création** (pas en fin de programme) : robuste face à un arrêt brutal. Clôt le chantier sûreté fichiers (config des racines, garde-fou symlink, nettoyage temp).

---

## [13/06/2026] — Garde-fou symlink pour les capacités fichiers

Le confinement des capacités de répertoire ne se limite plus au rejet lexical de `..`/chemins absolus : la cible réelle est **canonicalisée** (liens symboliques et jonctions Windows résolus) et vérifiée comme restant **sous la racine canonique**. Un lien pointant hors de la capacité est donc bloqué (`chemin hors de la capacité`) ; un lien interne reste autorisé.

Détail : `cap_resolve` compare le plus profond ancêtre *existant* de la cible à celui de la racine (`canonicalize` exige l'existence — un fichier à créer n'existe pas encore), la racine octroyée existant toujours. Reste une fenêtre TOCTOU théorique, acceptable pour le modèle coopératif (étanchéité adversariale → confinement OS type `cap-std`, non retenu).

Tests : escapade via symlink bloquée / lien interne autorisé (`#[cfg(unix)]`) ; vérifié sur Windows via une jonction. À venir : nettoyage des temp.

---

## [13/06/2026] — Racines fichiers configurées (`[files.roots]`)

Les capacités de répertoire peuvent désormais être **octroyées par le `minilang.toml`** (en plus de `FileSystem.tempDir()`) : des **racines nommées**, avec un mode d'accès.

```toml
[files.roots.data]
path = "data"            # relatif au minilang.toml ; doit exister au démarrage
mode = "read-write"

[files.roots.assets]
path = "assets"
mode = "read"            # défaut
```

```java
FileSystem fs = inject FileSystem;
ReadWriteDir data   = fs.rootRW("data").getValue();
ReadDir      assets = fs.root("assets").getValue();
fs.rootRW("assets");     // → Err : racine en lecture seule
```

- `root(nom)` → `Result<ReadDir, IoError>` (lecture) ; `rootRW(nom)` → `Result<ReadWriteDir, IoError>` (échoue si la racine est `read` ou inconnue).
- Les répertoires configurés **doivent exister au démarrage** (sinon erreur de configuration fatale) ; chemins canonicalisés. La config — hors du programme — octroie les racines : le code ne peut pas en forger.

Doc : `docs/configuration.md`, `docs/io.md`. Exemple : `examples/example_files_config/`. À venir : garde-fou symlinks, nettoyage des temp.

---

## [13/06/2026] — Accès fichiers par capacités (confinement)

Accès au système de fichiers **confiné** par capacités (modèle object-capability, cf. WASI/Capsicum), en alternative aux chemins bruts de `Files`.

- **`ReadDir`** (lecture) et **`ReadWriteDir extends ReadDir`** (+ écriture) : le **mode est dans le type**. Une fonction qui reçoit un `ReadDir` ne peut **pas compiler** une écriture — restriction de droits garantie à la compilation (rendue possible par l'héritage d'interface).
- **`FileSystem`** (service injectable) : seul à pouvoir **minter** une racine (`tempDir()` → répertoire temporaire frais) — pas d'autorité ambiante. Le code ne peut ensuite que **restreindre** via `sub`/`subRW`.
- On ne manipule **jamais de chemin absolu** : accès à des enfants relatifs ; `..` et chemins absolus rejetés (confinement). Les parents sont créés à l'écriture.
- `new Directory()` direct est **inerte** (racine non-forgeable).

```java
FileSystem fs = inject FileSystem;
ReadWriteDir root = fs.tempDir().getValue();
root.writeText("notes.txt", "bonjour");
ReadDir ro = root.sub("config");     // ro.writeText(...) ne compile pas
```

À venir : racine depuis `minilang.toml`, garde-fou symlinks, nettoyage des temp (marqueur `delete.me`). Modèle de menace : évasions accidentelles / code coopératif. Doc : `docs/io.md`, exemple : `examples/example_file_capabilities.mini`.

---

## [13/06/2026] — I/O fichiers en bloc (I/O phase 3b)

Classe utilitaire **injectable `Files`** (`minilang.io`) pour lire/écrire des fichiers **en bloc** — pas de flux, pas de handle à fermer. Les octets sont la donnée primitive ; la string est un décodage UTF-8 faillible.

```java
Files files = inject Files;
files.writeText("notes.txt", "bonjour");
Result<string, IoError> txt = files.readText("notes.txt");   // Err si UTF-8 invalide
Result<byte[], IoError> raw = files.readBytes("photo.png");
files.appendText("log.txt", "...");
bool present = files.exists("notes.txt");
files.delete("notes.txt");
```

Méthodes : `readBytes`/`readText`, `writeBytes`/`writeText` (écrasent), `appendBytes`/`appendText` (créent si absent), `exists` (→ `bool`), `delete`. Toutes (sauf `exists`) renvoient un `Result<_, IoError>`.

Choix : bloc seul (pas de streaming de fichiers — couvre l'essentiel sans gestion de ressources) ; accès disque libre pour l'instant (garde-fou de sûreté repoussé). Doc : `docs/io.md`, exemple : `examples/example_files.mini`.

---

## [13/06/2026] — Type `byte` et conversions string ↔ byte[] (I/O phase 3a)

Nouveau primitif **`byte`** : octet non signé (0–255), type de **stockage sans arithmétique** (seuls `==`/`!=` et les conversions). On crée un byte via `int.toByte()` (→ `Option<byte>`, `None` hors plage) et on le relit via `byte.toInt()`. `byte[]` est un tableau ordinaire, distinct de `int[]` (type-safe).

Conversions texte ↔ binaire via la classe utilitaire **injectable `Bytes`** (`minilang.io`), seul pont entre les deux mondes — pas de flux d'octets séparé :

```java
Bytes bytes = inject Bytes;
byte[] data = bytes.encodeUtf8("héllo");            // string -> octets UTF-8
Result<string, IoError> r = bytes.decodeUtf8(data); // octets -> string (Err si invalide)
```

Choix de design : pas d'arithmétique sur `byte` (calculs via `int`, narrowing explicite par `Option`), pas de littéral byte. L'I/O fichiers (texte + binaire en bloc) reste à faire (phase 3b). Doc : `docs/io.md`, exemple : `examples/example_byte.mini`.

---

## [13/06/2026] — Système d'entrée/sortie (phase 2 : entrée + bufferisation)

- **`StandardInput`** (`minilang.system`) : service injectable lisant le vrai stdin. `readLine()` (sans le saut de ligne, EOF = `Ok(None)`), `readChar()` (caractère Unicode), `readAll()` — builtins natifs.
- **`StringInput`** (`minilang.io`) : entrée en mémoire (`feed(string)`), double de test pour `bind Input to StringInput`. Écrite en minilang pur.
- **`BufferedWriter`** (`minilang.io`) : sortie bufferisée concrète enveloppant un `Output`, transmise au `flush()`. Écrite en minilang pur.

```java
StandardInput in = inject StandardInput;
match in.readLine().getValue() {
    Option::Some(ligne) => { /* ... */ }
    Option::None        => { /* EOF */ }
}
```

Exemple : `examples/example_stdin.mini` (alimenté par pipe). Doc : `docs/io.md`.

---

## [13/06/2026] — Système d'entrée/sortie (phase 1 : sorties)

Packages **`minilang.io`** (interfaces `Output`, `Flushable`, `BufferedOutput extends Output, Flushable`, `Input`, plus `IoError`, `Unit`, `StringOutput`) et **`minilang.system`** (`StandardOutput`, `StandardError`).

- **Une seule hiérarchie, orientée texte** : l'unité d'I/O est la `string` (UTF-8), pas de flux d'octets séparé (binaire reporté à une phase ultérieure).
- **Erreurs explicites** : écriture/flush → `Result<Unit, IoError>`, lecture → `Result<Option<string>, IoError>` (EOF = `Ok(None)`). Le `Result` peut être ignoré (instruction simple) ou traité.
- **`StandardOutput` / `StandardError`** : services injectables écrivant sur stdout/stderr (méthodes natives).
- **`StringOutput`** : capture en mémoire (équivalent `StringWriter`) — combinée aux modules de binding, elle rend le code d'I/O testable sans le modifier (`bind Output to StringOutput`).

```java
StandardOutput out = inject StandardOutput;
out.writeLine("bonjour");
```

Documentation : `docs/io.md`, exemple : `examples/example_io.mini`. Phase 2 à venir : `StandardInput` (lecture stdin). `print` est conservé.

---

## [13/06/2026] — Héritage d'interface (`interface Sub extends A, B`)

Une interface peut désormais en étendre une ou plusieurs. Une classe (ou un record) qui implémente une interface doit fournir les méthodes de celle-ci **et de tous ses parents** (transitif) ; une sous-interface est un sous-type de ses parents ; la résolution de méthode remonte la chaîne des parents.

```java
interface Animal { string name(); }
interface Pet extends Animal { string owner(); }   // hérite de name()

class Dog implements Pet {
    string name()  { return "Rex"; }
    string owner() { return "Alice"; }
}
Animal a = new Dog();   // Dog -> Pet -> Animal
```

Héritage multiple et diamant autorisés ; cycle d'héritage = erreur de compilation. Première brique du futur système d'I/O (`BufferedOutput extends Output`). Limitation : les arguments de type sur un parent générique sont ignorés.

---

## [12/06/2026] — Correction du stack overflow de la CLI en mode debug

Le binaire plantait avec « thread 'main' has overflowed its stack » en mode debug sur Windows dès le parsing (stack frames profonds du parser chumsky, pile du thread principal limitée à 1 Mo). Le travail s'exécute désormais dans un thread dédié avec une pile de 16 Mo — `cargo run -- fichier.mini` fonctionne en debug comme en release.

---

## [12/06/2026] — La CLI embarque la bibliothèque standard

Le binaire (`mini_parser fichier.mini` et `mini_parser test`) préfixe désormais la stdlib au programme, comme le font les API de test Rust — `Option`, `Result`, les collections (`ArrayList`, `HashMap`, …) et `obj.equals()` fonctionnent maintenant en ligne de commande. L'affichage de l'AST reste limité aux déclarations du fichier utilisateur.

Au passage :
- correction d'une incohérence typechecker/interpréteur : une classe utilisateur a maintenant priorité sur un record stdlib du même nom (ex. `Pair`) dans les deux composants ;
- `example.mini` et `example_optional.mini` mis en conformité avec la règle des champs privés (accès via getters) — tous les exemples passent désormais le typecheck via la CLI.

---

## [12/06/2026] — Système de tests (`test`, assertions, runner)

Fonctions de test intégrées au langage et runner en ligne de commande.

```java
test void additionSimple() {
    assertEquals(add(2, 3), 5);
}
```

- **`test void nom() { ... }`** : fonction de test (void, sans paramètres — vérifié au typecheck). `main` devient **optionnel** pour les fichiers de tests (toujours requis en exécution normale).
- **Assertions builtin** typées : `assertTrue`, `assertFalse`, `assertEquals`, `assertNotEquals`, `fail` — types vérifiés à la compilation.
- **`mini_parser test [fichier|répertoire]`** : exécute les tests (défaut : `[tests] dir`, sinon `tests/`). Chaque test tourne dans un interpréteur neuf — **conteneur DI réinitialisé**, les singletons ne fuient pas entre tests. Code de sortie non-zéro si échec.
- **`[tests]` dans minilang.toml** : `dir` (répertoire) et `modules` (profil DI des tests → les mocks, sans toucher au code testé).
- **`[sources] include`** : fichiers partagés (sans `main`) préfixés au fichier exécuté, en mode run comme en mode test — les tests peuvent référencer le code de l'application.

---

## [12/06/2026] — Fichier de configuration de projet (`minilang.toml`)

Fichier **optionnel** à la racine du projet, découvert en remontant les répertoires depuis le fichier source (ou le répertoire courant). Absent → comportement par défaut inchangé. Priorité : CLI > `minilang.toml` > défauts. Un fichier présent mais invalide (TOML cassé, clé inconnue) est une erreur fatale.

```toml
[project]
name = "mon-appli"
main = "src/app.mini"       # permet de lancer mini_parser sans argument

[di]
modules = ["ProdModule"]    # profil DI : seuls ces modules de binding sont actifs

[runtime]
log = "info"                # RUST_LOG prioritaire
```

`[di] modules` apporte les **profils d'injection** : le code déclare `ProdModule` et `TestModule`, la config choisit — on bascule sur les mocks sans toucher au code. Documentation : `docs/configuration.md`, exemple : `examples/example_config/`.

---

## [12/06/2026] — DI phase 2 : modules de binding, `with`, `transient`

Le bloc `module` centralise la configuration du conteneur d'injection — c'est lui qui permet d'échanger les implémentations sans toucher au code (profils test/prod). Plusieurs modules coexistent, leurs bindings sont fusionnés.

```java
module AppModule {
    bind Logger to FileLogger;                 // choisit l'implémentation (lève l'ambiguïté)
    bind HttpClient with ("https://api", 30);  // valeurs de configuration du constructeur
    bind Repo to SqlRepo with ("jdbc:...");    // les deux combinés
}
```

- Les paramètres de constructeur non-service (primitifs, classes ordinaires) deviennent des **slots de configuration**, remplis dans l'ordre par les valeurs du `with`.
- **`transient service class X`** : nouvelle instance à chaque injection au lieu d'un singleton. Un singleton ne peut pas dépendre d'un transient (dépendance captive — erreur de compilation).
- Nouvelles erreurs au typecheck : binding dupliqué, `bind` sans effet, cible inconnue ou n'implémentant pas l'interface, arité/types du `with` incorrects, dépendance captive, `transient` sans `service`.

---

## [12/06/2026] — Injection de dépendances (`service` / `inject`)

Conteneur d'injection de dépendances **sans réflexion**, entièrement validé à la compilation. `service class X` marque une classe gérée par le conteneur ; ses dépendances sont les paramètres de son constructeur. `inject T` (autorisé dans `main` et les fonctions de haut niveau) retourne le **singleton** du service `T` — ou de l'unique service implémentant l'interface `T`.

Erreurs détectées au typecheck : binding manquant, binding ambigu (plusieurs implémentations), cycle de dépendances, paramètre non injectable, service générique, constructeurs multiples. L'exécution ne peut pas échouer.

```java
interface Logger { void log(string msg); }
service class ConsoleLogger implements Logger {
    void log(string msg) { print(msg); }
}
service class UserService {
    Logger logger;
    UserService(Logger logger) { this.logger = logger; }
}
int main() {
    UserService s = inject UserService;  // ConsoleLogger câblé automatiquement
    return 0;
}
```

---

## [02/06/2026] — Visibilité des membres + type `record`

Les champs de classe sont désormais **toujours privés** (accès autorisé depuis la même classe, interdit depuis l'extérieur). Les méthodes sont **publiques par défaut** ; `private` et `protected` permettent de restreindre l'accès.

Ajout du type **`record`** : agrégat immuable à champs positionnels avec getters, `copy`, `equals`, `toString` et `hashCode` générés automatiquement. Hérite de la classe `Record`. `Pair<A,B>` migré de enum à record.

```java
record Point(int x, int y) {}

Point p  = new Point(1, 2);
Point p2 = p.copy(Option<int>::None, Option<int>::Some(10)); // x inchangé, y=10
p.getX();        // 1
p.toString();    // "Point(x=1, y=2)"
p.x;             // ERREUR : champ privé
```

---

## [30/05/2026] — Système d'immutabilité en quatre phases

Qualificateurs `readonly` / `immutable` sur les variables, mot-clé `mutable` sur les méthodes de classe, marqueur `mut` sur les types, propagation transitive du qualificateur dans les appels enchaînés, et contraintes `immutable K` sur les paramètres de type génériques.

```java
mut class Counter {
    int value;
    mutable void increment() { value = value + 1; }
    int get() { return value; }
}
readonly Counter rc = new Counter();
rc.increment();  // ERREUR : méthode mutable sur readonly
rc.get();        // OK
```

---

## Fonctionnalités du langage (historique)

### Collections et itération

- **`Iterator<T>` / `Iterable<T>`** : interfaces d'itération standard.
- **`for (T x in collection)`** : syntaxe de boucle sur tout `Iterable<T>`.
- **`forEach(fn(T) -> void)`** sur `Iterator`, `List`, `Set`, `Map`.
- **`List<T>` / `Set<T>` / `Map<K,V>`** : interfaces pour les collections.
- **`ArrayList<T>`** : implémentation de `List<T>` avec tableau redimensionnable.
- **`HashMap<K,V>`** : implémentation de `Map<K,V>`, builtin natif.
- **`HashSet<T>`** : implémentation de `Set<T>` via `HashMap<T, bool>`, écrite en minilang.
- `HashMap.keys()` et `HashMap.entries()` retournent `List` (pas `ArrayList`).

### Tableaux (Array)

- **`T[]`** : type tableau générique.
- **`new T[n]`** : création avec taille.
- **`new T[n](fill)`** : création avec valeur de remplissage initiale.
- **`arr[i]`** : accès en lecture — retourne `Option<T>` (jamais de panique sur index hors bornes).
- **`arr.set(i)`** : accès en écriture sûr — retourne `Option<RefArray<T>>`.
- **`RefArray<T>`** : référence sur une case de tableau ; `.set(val)` et `.get()` pour lire/écrire.
- La syntaxe `arr[i] = val` a été supprimée ; il faut utiliser `arr.set(i)`.

### String — méthodes sûres

- **`charAt(int)`** → `Option<char>` (plus de panique sur index hors bornes).
- **`indexOf(string)`** → `Option<int>` (plus de sentinelle `-1`).
- **`split(string)`** → `List<string>`.
- 8 méthodes réécrites en **minilang pur** à partir des 7 builtins irréductibles :
  `isEmpty`, `contains`, `startsWith`, `endsWith`, `toUpperCase`, `toLowerCase`, `trim`, `replace`.

### Interface HashCode

- **`HashCode`** : interface avec `int hashCode()`.
- Implémentée par tous les types primitifs :
  - `int` → la valeur elle-même
  - `bool` → `0` ou `1`
  - `char` → point de code Unicode
  - `string` → hash polynomial (DefaultHasher)
  - `float` / `double` → bits IEEE 754

### Types de base

- **`int`**, **`float`**, **`double`**, **`bool`**, **`char`**, **`string`** : types primitifs.
- **`void`** : absence de valeur de retour.
- Méthodes builtin sur chaque primitif : `Integer`, `Boolean`, `Character`, `Float`, `Double`, `String` (classes wrapper).

### Types algébriques et génériques

- **`Option<T>`** : `Some(T)` / `None` — valeur optionnelle.
- **`Result<T, E>`** : `Ok(T)` / `Err(E)` — résultat ou erreur.
- **`Either<L, R>`** : `Left(L)` / `Right(R)` — union discriminée.
- **`Pair<A, B>`** : `Of(A first, B second)` — paire de valeurs.
- **Enums génériques** avec variants paramétrés et méthodes.
- **Classes génériques** avec paramètres de type.

### Orienté objet

- **Classes** avec champs, constructeurs, méthodes, héritage (`extends`).
- **Interfaces** avec signatures de méthodes (`implements`).
- **`Object`** : classe racine implicite avec `equals(Object)`.
- Méthodes en minilang pur dans les classes et enums.

### Contrôle du flux

- `if` / `else`, `while`, `do while`, `for (init; cond; update)`, `for (T x in iter)`.
- **`match expr { Variant => { ... } }`** : filtrage par motif sur les enums.
- `break`, `continue`, `return`.

### Lambdas et fonctions

- **Lambdas** : `fn(x) { return x + 1; }` — fermetures capturant l'environnement.
- **Type `fn(T1, T2) -> R`** : type annoté pour les lambdas.
- **Fonctions de haut niveau** : déclarées en dehors des classes.

### Autres

- **`print`** : affichage.
- **`panic(msg)`** : erreur fatale à l'exécution.
- **Package et imports** : `package minilang.lang;` / `import minilang.collection.List;`.
- **Commentaires** : `//` (ligne), `/* */` (bloc), `/** */` (doc).
- **Chaînes multi-lignes** et caractères d'échappement (`\n`, `\t`, `\"`, `\\`).
- **Alias de type** : `type Adder = fn(int, int) -> int;`.
