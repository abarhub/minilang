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
