# Changelog — minilang

Toutes les évolutions notables du langage sont documentées ici.

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
