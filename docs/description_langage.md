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
