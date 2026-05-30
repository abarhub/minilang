# Changelog — minilang

Toutes les évolutions notables du langage sont documentées ici.

---

## [30/05/2026] — Système d'immutabilité

Ajout d'un système d'immutabilité en quatre phases, vérifié statiquement par le typechecker.

### Phase 1 — Qualificateurs de variables et méthodes mutables

- Nouveau qualificateur **`readonly`** sur les variables : vue en lecture seule, ne peut pas appeler de méthode `mutable`.
- Nouveau qualificateur **`immutable`** sur les variables : immuable, ne peut pas appeler de méthode `mutable`.
- Nouveau mot-clé **`mutable`** sur les méthodes de classe : signale qu'une méthode modifie l'état de l'objet.
- Règle : dans une méthode non-`mutable`, `this` est traité comme `readonly` — impossible d'y appeler une méthode `mutable`.
- Annotation de la stdlib : `List.add/set/remove/clear`, `Set.add/remove/clear`, `Map.put/remove/clear` marquées `mutable`.

```java
mut class Counter {
    int value;
    mutable void increment() { value = value + 1; }
    int get() { return value; }
}

Counter c = new Counter();
readonly Counter rc = c;
rc.increment();   // ERREUR : méthode mutable sur readonly
rc.get();         // OK
```

### Phase 2 — Audit des classes avec `mut`

- Nouveau mot-clé **`mut`** devant `class` et `interface` : marque le type comme participant au système d'immutabilité.
- Règle : une variable `readonly X` ou `immutable X` n'est autorisée que si `X` est déclaré `mut`.
- Les **enums** sont `mut` implicitement (ils sont immuables par nature).
- Les **primitifs** (`int`, `bool`, `char`, `string`, `float`, `double`) sont des types valeur, toujours autorisés.
- Annotation de la stdlib : toutes les classes et interfaces portent maintenant `mut` (`Object`, `String`, `Integer`, `Boolean`, `Character`, `Float`, `Double`, `RefArray`, `ArrayList`, `HashMap`, `HashSet`, etc.).

```java
mut class Point { int x; int getX() { return x; } }
class Helper    { int compute(int x) { return x * 2; } }

immutable Point p  = new Point();   // OK — Point est mut
readonly  Helper h = new Helper();  // ERREUR — Helper n'est pas mut
```

### Phase 3 — Propagation transitive dans les appels enchaînés

- Le qualificateur d'un récepteur se propage automatiquement au résultat d'un appel de méthode non-mutable, si le type de retour est un type référence.
- Les **types valeur** (primitifs) stoppent la propagation.

```java
readonly Outer ro = ...;
ro.getInner().reset();          // ERREUR : reset() est mutable,
                                //          Inner hérite readonly de ro

ro.getInner().get();            // OK : get() n'est pas mutable
int n = ro.getCount();          // OK : getCount() retourne int (type valeur)

// Propagation sur trois niveaux :
readonly Root rr = ...;
rr.getMid().getLeaf().set(1);   // ERREUR
```

### Phase 4 — Contraintes de type params

- Les paramètres de type peuvent être annotés **`immutable K`** (ou `readonly K`) dans les déclarations de classe, interface ou enum.
- Cela impose que le type fourni pour ce paramètre soit déclaré `mut` (ou soit un primitif/enum).

```java
mut class Map<immutable K, V> {
    mutable void put(K key, V value) { ... }
}

mut class Point { ... }
class Helper    { ... }

Map<Point,  int> m1 = ...;   // OK — Point est mut
Map<Helper, int> m2 = ...;   // ERREUR — Helper n'est pas mut
Map<string, int> m3 = ...;   // OK — string est un primitif
Map<Option<int>, string> m4; // OK — les enums sont toujours mut
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
