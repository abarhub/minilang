# Langage Minilang

Le langage Minilang est un langage de programmation minimaliste conçu pour illustrer les concepts de sécurité de la programmation. 
Il est conçu pour être sécurisé, tout en permettant de créer des programmes complexes.

# Types

## Primitives

* bool: le type booléen
* int: le type entier 32 bits
* float: le type flottant 32 bits
* double: le type double
* string: le type chaîne de caractères
* char: le type caractère. Correspond à un caractère unicode
* void: le type void. absent de valeur. Utilisé pour les fonctions qui ne retournent rien

|Classe	|Type	| Sélection de méthodes                                                                                                             |
|-----|-----|-----------------------------------------------------------------------------------------------------------------------------------|
|Boolean|	bool| 	toString, and, or, not, equals                                                                                                   |
|Integer|	int| 	abs, min, max, pow, isEven, isOdd, compareTo, toBinaryString, toFloat…                                                           |
|Float|	float| 	abs, floor, ceil, round, isNaN, min, max, toInt…                                                                                 |
|Double|	double| 	idem Float + toFloat                                                                                                             |
|String|	string| 	length, isEmpty, charAt, contains, substring, toUpperCase, toLowerCase, startsWith, endsWith, indexOf, trim, replace, split…     |
|Character|	char| 	isLetter, isDigit, isWhitespace, isUpperCase, isLowerCase, toUpperCase, toLowerCase, toInt, toString                             |

Les opérations arithmétiques sont disponibles pour les types int et float.
Exemples :
```
// Arithmétique
+ - * / % **          // ** = puissance

// Comparaison
== != < <= > >=

// Logique
&& || !

// Navigation sûre (sur Option<T>)
obj?.field
obj?.method()
valeur ?? defaut
```

## Types complexes

### Tableau

Exemples :
```
int[] a = new int[5];              // taille fixe, valeurs par défaut
int[] b = new int[]{1, 2, 3};     // avec valeurs initiales
int x  = b[0];                     // accès par index
b[1]   = 99;                       // affectation par index
```

### Fonction

Exemples :
```
fn(int, int) -> int add = (a, b) => a + b;
fn double = x => x * 2;
fn block  = (x) => { return x + 1; };
```

### Classe

Les classes supportent l'héritage (extends) et les interfaces (implements).

Exemple :
```
class Point {
int x;
int y;

    Point(int x, int y) {
        this.x = x;
        this.y = y;
    }

    int distSq() {
        return this.x * this.x + this.y * this.y;
    }
}
```

### Visibilité des membres d'une classe

#### Champs — toujours privés

Les champs d'une classe sont **toujours privés** : aucun mot-clé n'est nécessaire ni autorisé.
L'accès est permis depuis n'importe quelle méthode de la même classe (y compris sur une autre instance du même type), mais interdit depuis l'extérieur.

```java
mut class Counter {
    int value;                         // privé implicitement

    int getValue() { return value; }   // OK — même classe, via this
    bool equals(Counter other) {
        return this.value == other.value;  // OK — même classe, autre instance
    }
}

int main() {
    Counter c = new Counter();
    int v = c.value;   // ERREUR : champ privé, inaccessible depuis l'extérieur
    return 0;
}
```

#### Méthodes — publiques par défaut

Sans modificateur, une méthode est **publique** (accessible de partout).
Deux mots-clés optionnels permettent de restreindre l'accès :

| Modificateur | Placement | Accessible depuis |
|---|---|---|
| _(aucun)_ | — | Partout |
| `protected` | avant le type de retour | La classe déclarante et ses sous-classes |
| `private` | avant le type de retour | La classe déclarante uniquement |

```java
mut class Animal {
    string name;

    // public (défaut) — accessible partout
    string getName() { return name; }

    // protected — accessible depuis Animal et ses sous-classes
    protected string buildLabel() { return name; }

    // private — accessible uniquement dans Animal
    private bool validate() { return true; }
}

mut class Dog extends Animal {
    string describe() { return this.buildLabel(); } // OK — sous-classe
    void test()       { this.validate(); }          // ERREUR — private
}

int main() {
    Animal a = new Animal();
    a.getName();     // OK — public
    a.buildLabel();  // ERREUR — protected
    a.validate();    // ERREUR — private
    return 0;
}
```

Les modificateurs se placent avant `mutable` lorsque les deux sont combinés :

```java
mut class Counter {
    int value;
    private mutable void reset()  { value = 0; }       // private + mutable
    mutable void increment()      { value = value + 1; }
    int getValue()                { return value; }
}
```

### Record

Un record est un agrégat de données immuable. Les champs sont déclarés dans les parenthèses ; getters, `copy`, `equals`, `toString` et `hashCode` sont générés automatiquement. Les méthodes `mutable` sont interdites. Un record hérite implicitement de `Record` et peut implémenter des interfaces.

```java
record Point(int x, int y) {}

Point p  = new Point(3, 4);
int   vx = p.getX();          // getter généré
Point p2 = p.copy(Option<int>::None, Option<int>::Some(10));  // y remplacé, x inchangé
string s = p.toString();      // "Point(x=3, y=4)"
bool  eq = p.equals(p2);
int   h  = p.hashCode();
```

Record générique :
```java
record Pair<A, B>(A first, B second) {}

Pair<int, string> p = new Pair<int, string>(1, "hello");
int    f = p.getFirst();
string s = p.getSecond();
```

### Classe générique

Exemple :
```
Option<int>          // optionnel
Result<int, string>  // succès ou erreur
Either<int, string>  // l'un ou l'autre
Pair<int, string>    // paire de valeurs
Array<T>             // tableau (voir ci-dessus)
```

### Interface

Exemple :
```
interface Printable {
string toString();
}
```

Une interface peut **en étendre une ou plusieurs** avec `extends`. Une classe qui implémente une interface doit fournir les méthodes de celle-ci **et de tous ses parents** (transitif). Une sous-interface est un sous-type de ses parents.

```java
interface Animal { string name(); }
interface Pet extends Animal {        // Pet hérite de name()
    string owner();
}

class Dog implements Pet {            // doit fournir name() ET owner()
    string name()  { return "Rex"; }
    string owner() { return "Alice"; }
}

Pet    p = new Dog();
Animal a = p;                          // Pet est sous-type d'Animal
```

L'héritage multiple et le diamant sont autorisés (`interface D extends B, C`) ; un cycle d'héritage est une erreur de compilation. Limitation actuelle : les arguments de type sur un parent générique (`extends Base<int>`) sont ignorés.

### Entrées/sorties

Les entrées/sorties sont fournies par les packages `minilang.io` (interfaces `Output`, `Flushable`, `BufferedOutput`, `Input` ; erreurs via `Result<Unit, IoError>`) et `minilang.system` (`StandardOutput`, `StandardError`, services injectables). Hiérarchie unique orientée texte (pas de flux d'octets séparé). Détails et exemples : voir `docs/io.md`.

```java
StandardOutput out = inject StandardOutput;
out.writeLine("bonjour");
```

### Enum

Exemple :
```
enum Color {
Red, Green, Blue;

    string name() {
        match this {
            Color::Red   => { return "rouge"; }
            Color::Green => { return "vert"; }
            Color::Blue  => { return "bleu"; }
        }
    }
}
```

Exemple enum générique :
```
enum Box<T> {
Full(T value),
Empty;

    bool hasValue() {
        match this {
            Box::Full(v) => { return true; }
            Box::Empty   => { return false; }
        }
    }
}
```

# Injection de dépendances

Le mot-clé `service` marque une classe gérée par le conteneur d'injection de dépendances.
Les dépendances d'un service sont **les paramètres de son constructeur** : le conteneur les résout et les fournit automatiquement. Aucune réflexion n'est utilisée — tout le câblage est résolu à la compilation.

L'expression `inject T` récupère l'instance du service `T`. Si `T` est une interface, le conteneur injecte l'unique service qui l'implémente. Les services sont des **singletons** : chaque `inject` retourne la même instance.

```java
interface Logger { void log(string msg); }

service class ConsoleLogger implements Logger {
    void log(string msg) { print(msg); }
}

service class UserService {
    Logger logger;
    UserService(Logger logger) { this.logger = logger; }   // dépendance injectée
    void hello() { logger.log("hello"); }
}

int main() {
    UserService s = inject UserService;   // câble ConsoleLogger → UserService
    s.hello();
    return 0;
}
```

## Modules de binding

Un bloc `module` centralise la configuration du conteneur. C'est lui qui permet d'échanger les implémentations sans toucher au code des classes (profil de test avec des mocks, profil de prod, …). Plusieurs modules peuvent coexister ; leurs bindings sont fusionnés.

```java
module AppModule {
    bind Logger to FileLogger;                         // choisit l'implémentation
    bind HttpClient with ("https://api", 30);          // valeurs de configuration
    bind Repo to SqlRepo with ("jdbc:...");            // les deux combinés
}
```

- **`bind Iface to Service;`** — choisit l'implémentation d'une interface. Obligatoire dès qu'une interface injectée a plusieurs implémentations service (sinon binding ambigu) ; le binding s'applique partout : `inject Iface` et dépendances de constructeur.
- **`bind Service with (val, …);`** — fournit les **paramètres de configuration** du constructeur. Les paramètres dont le type est une interface ou un service sont injectés ; tous les autres (primitifs, classes ordinaires, …) sont des slots de configuration, remplis dans l'ordre par les valeurs du `with`.

```java
service class HttpClient {
    Logger logger;     // injecté (interface)
    string baseUrl;    // configuration — fourni par le with
    int    timeout;    // configuration — fourni par le with
    HttpClient(Logger logger, string baseUrl, int timeout) { … }
}

module AppModule {
    bind HttpClient with ("https://api", 30);
}
```

## Scope `transient`

Par défaut un service est un **singleton**. Le mot-clé `transient` (placé avant `service`) crée une **nouvelle instance à chaque injection** :

```java
transient service mut class RequestContext {
    …
}
```

Un service singleton ne peut pas dépendre d'un service `transient` (dépendance captive : le singleton figerait son instance) — c'est une erreur de compilation.

## Règles

Toutes vérifiées **à la compilation** (l'exécution ne peut pas échouer) :

| Règle | Erreur si violée |
|---|---|
| Un service a au plus un constructeur | `au plus un constructeur` |
| Un service ne peut pas être générique | `ne peut pas être générique` |
| Les paramètres non-service du constructeur sont couverts par un `bind … with (…)` | `n'est pas injectable — fournissez sa valeur via bind…` |
| Le nombre et les types des valeurs `with` correspondent aux paramètres de configuration | `valeur(s) fournie(s) mais…` / `type incompatible` |
| Chaque interface injectée a une implémentation choisie (unique ou via `bind … to …`) | `Aucun service n'implémente…` / `Binding ambigu…` |
| Un binding cible une interface connue et un service qui l'implémente | `n'implémente pas…` / `doit être déclarée service` |
| Pas deux bindings (ou deux `with`) pour la même cible | `Binding dupliqué…` / `Valeurs with dupliquées…` |
| Le graphe de dépendances est acyclique | `Cycle de dépendances entre services` |
| Un singleton ne dépend pas d'un service `transient` | `dépendance captive` |
| `transient` ne s'applique qu'aux services | `transient nécessite service` |
| `inject` n'est autorisé que dans `main` et les fonctions de haut niveau | `'inject' n'est autorisé que dans…` |

Un service peut être `mut` (`service mut class Stats { … }`) et avoir des méthodes `mutable` ; comme les injections partagent le même singleton, l'état est visible par tous les consommateurs.

Avoir plusieurs services implémentant la même interface n'est une erreur que si cette interface est effectivement injectée quelque part sans `bind … to …` (le binding est alors ambigu).

# Fonctions

Une fonction permet de faire un traitement avec des instructions.
Elle peut être dans une classe ou hors de la classe.
La fonction doit retourner un type. S'il n'y a pas de valeur à retourner, la méthode doit retourner void.
La méthode main est le point d'entrée du programme. Elle doit retourner int.

Exemple :
```
int add(int a, int b) {
    return a + b;
}
```


Exemple de méthode main :
```
int main() {
    // ...
    return 0;
}
```

# Tests

Une fonction de haut niveau préfixée par `test` est une fonction de test : `void`, sans paramètres. Un fichier de tests n'a pas besoin de `main` (qui reste obligatoire pour l'exécution normale).

```java
int add(int a, int b) { return a + b; }

test void additionSimple() {
    assertEquals(add(2, 3), 5);
}

test void comparaisons() {
    assertTrue(1 < 2);
    assertFalse(1 > 2);
    assertNotEquals("a", "b");
}
```

Les assertions sont des fonctions builtin, vérifiées par le typechecker :

| Assertion | Vérifie | Erreur de compilation si |
|---|---|---|
| `assertTrue(bool)` | la condition est vraie | argument non-bool |
| `assertFalse(bool)` | la condition est fausse | argument non-bool |
| `assertEquals(a, b)` | `a` égale `b` | types incomparables |
| `assertNotEquals(a, b)` | `a` diffère de `b` | types incomparables |
| `fail(string)` | échoue toujours avec le message | argument non-string |

Le runner s'invoque avec `mini_parser test [fichier|répertoire]` (par défaut : le répertoire `[tests] dir` du `minilang.toml`, sinon `tests/`). Chaque test s'exécute dans un **interpréteur neuf** : environnement vierge et conteneur d'injection réinitialisé — les singletons ne fuient pas d'un test à l'autre. Un échec (assertion, `panic`, erreur runtime) n'arrête pas les tests suivants ; le code de sortie est non-zéro si au moins un test échoue.

```
$ mini_parser test
── tests/user_service_test.mini
test greetUtiliseLeRepoInjecte ... ok
test lesAssertionsDeBase ... ok

Résultat : 2 test(s), 0 échec(s)
```

Combiné aux modules de binding et au `minilang.toml`, le runner applique le **profil DI de test** (`[tests] modules`) : le code injecte `Repo`, la prod binde `SqlRepo`, les tests bindent `FakeRepo` — sans toucher au code testé. Voir `docs/configuration.md` et l'exemple `examples/example_config/`.

# Variable

Exemples :
```
int n = 42;
string s = "hello";
int[] arr = new int[]{1, 2, 3};
```

# Alias de type

Exemple :
```
type Adder = fn(int, int) -> int;
```

# Instructions

Exemples :
```
if (x > 0) { ... } else { ... }
while (condition) { ... }
do { ... } while (condition);
for (int i = 0; i < 10; i = i + 1) { ... }
break;
continue;
return valeur;
```
Exemple de pattern matching :
```
match r {
Result::Ok(v)  => { return v; }
Result::Err(e) => { return -1; }
_              => { return 0; }    // wildcard
}
```

Exemple d'affichage :
```
print("valeur :", n);
```

# Structures de controle

## if

## while

## for

# classes standard

* String
* Array
* Integer
* Float
* Boolean
* Character
* Float
* Double
* Option
* Pair
* Result
* Either

## Type optionnel (T?)

Exemples :
```
int? maybe = Option<int>::Some(42);
int? none  = Option<int>::None;
int  val   = maybe ?? 0;           // null coalescing
```

# Collections

Les collections se trouvent dans le package `minilang.collection`. Elles doivent être importées avant d'être utilisées.

## List / ArrayList

`List<T>` est une interface représentant une liste ordonnée d'éléments.
`ArrayList<T>` est l'implémentation fournie par la bibliothèque standard. Sa taille croît automatiquement.

```
import minilang.collection.List;
import minilang.collection.ArrayList;

// Création
List<int> liste = new ArrayList<int>();

// Ajout d'éléments
liste.add(10);
liste.add(20);
liste.add(30);

// Accès par index — retourne Option<T>
Option<int> opt = liste.get(1);    // Some(20)
int val = liste.get(1).get();      // 20 (lève une erreur si absent)
int val2 = liste.get(99) ?? 0;    // 0 si hors limites

// Modification — la lambda n'est appelée que si l'index est valide
bool ok = liste.set(0, () => 99); // true si modifié, false si hors limites

// Recherche
bool found   = liste.contains(20);           // true
Option<int> idx = liste.indexOf(20);         // Some(1)
Option<int> found2 = liste.find(20);         // Some(20)

// Suppression
liste.remove(0);     // supprime l'élément à l'index 0

// Taille
int n = liste.size();
bool empty = liste.isEmpty();

// Vidage
liste.clear();

// Affichage
print(liste.toString());
```

## Set / HashSet

`Set<T>` est une interface représentant un ensemble sans doublons.
`HashSet<T>` est l'implémentation fournie par la bibliothèque standard.

```
import minilang.collection.Set;
import minilang.collection.HashSet;

// Création
Set<string> ensemble = new HashSet<string>();

// Ajout — retourne true si l'élément a été ajouté, false s'il existait déjà
bool added = ensemble.add("alice");   // true
bool dup   = ensemble.add("alice");   // false (déjà présent)
ensemble.add("bob");
ensemble.add("charlie");

// Recherche
bool present = ensemble.contains("bob");    // true
bool absent  = ensemble.contains("dave");   // false

// Suppression — retourne true si l'élément existait
bool removed = ensemble.remove("bob");      // true

// Taille
int n = ensemble.size();
bool empty = ensemble.isEmpty();

// Vidage
ensemble.clear();

// Affichage
print(ensemble.toString());
```

## Map / HashMap

`Map<K, V>` est une interface représentant une table associative (clé → valeur).
`HashMap<K, V>` est l'implémentation fournie par la bibliothèque standard.

```
import minilang.collection.Map;
import minilang.collection.HashMap;
import minilang.collection.ArrayList;

// Création
Map<string, int> scores = new HashMap<string, int>();

// Insertion / mise à jour
scores.put("alice", 100);
scores.put("bob",   85);
scores.put("alice", 110);   // remplace la valeur existante

// Accès — retourne Option<V>
Option<int> opt  = scores.get("alice");    // Some(110)
int val          = scores.get("alice").get(); // 110
int missing      = scores.get("dave") ?? 0;  // 0 si absent

// Vérification de présence
bool exists = scores.containsKey("bob");    // true

// Suppression — retourne true si la clé existait
bool removed = scores.remove("bob");        // true

// Liste des clés
ArrayList<string> cles = scores.keys();
int i = 0;
while (i < cles.size()) {
    print(cles.get(i).get());
    i = i + 1;
}

// Taille
int n = scores.size();
bool empty = scores.isEmpty();

// Vidage
scores.clear();

// Affichage
print(scores.toString());
```
