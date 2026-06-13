# Entrées / sorties

Le système d'I/O est réparti en deux packages :

- **`minilang.io`** — les interfaces générales et les types de support ;
- **`minilang.system`** — les flux standard du processus (stdout, stderr).

Toutes ces classes sont dans la bibliothèque standard : accessibles sans import.

## Principe : une seule hiérarchie, orientée texte

Contrairement à Java (flux d'octets `InputStream` **et** flux de caractères `Reader`), minilang a **une seule hiérarchie, orientée texte**. L'unité d'I/O est la `string` (UTF-8) ; il n'y a pas de flux d'octets séparé. Le binaire passe par le type `byte` et des **conversions** `string ↔ byte[]`, sans hiérarchie parallèle.

## Binaire : le type `byte` et les conversions

`byte` est l'octet non signé (0–255), un type de **stockage** sans arithmétique. On le construit depuis un `int` et on le relit en `int` :

```java
byte b = (200).toByte().get();   // int.toByte() -> Option<byte> (None si hors 0..255)
int  n = b.toInt();              // toujours valide
```

La classe utilitaire **injectable `Bytes`** est le seul pont texte ↔ binaire :

```java
Bytes bytes = inject Bytes;

byte[] data = bytes.encodeUtf8("héllo");          // string -> octets UTF-8 (total)
Result<string, IoError> r = bytes.decodeUtf8(data); // octets -> string (Err si UTF-8 invalide)
```

`byte[]` est un tableau ordinaire (`new byte[n]`, `get`/`set`…), distinct de `int[]`. Il n'y a **pas** de flux binaire : pour lire/écrire des octets en masse, on convertira depuis/vers `byte[]` (l'I/O fichiers binaire viendra dans une phase ultérieure).

## Result : les erreurs sont explicites

Chaque opération d'I/O renvoie un `Result` :

- écriture / flush → `Result<Unit, IoError>` (`Unit` = succès sans valeur, l'équivalent du `()` de Rust) ;
- lecture → `Result<Option<string>, IoError>` : `Ok(Some(x))` = donnée, `Ok(None)` = fin de flux (EOF), `Err(e)` = erreur réelle.

Le `Result` peut être **ignoré** (appel en instruction simple) pour le code rapide, ou **traité** explicitement :

```java
StandardOutput out = inject StandardOutput;

out.writeLine("rapide");                       // Result ignoré

Result<Unit, IoError> r = out.writeLine("soigné");
if (r.isErr()) {
    // r.getError().message() décrit l'erreur
}
```

## Interfaces (`minilang.io`)

```java
mut interface Output {
    mutable Result<Unit, IoError> write(string s);       // sans saut de ligne
    mutable Result<Unit, IoError> writeLine(string s);   // avec saut de ligne
}

mut interface Flushable {
    mutable Result<Unit, IoError> flush();
}

// Héritage d'interface : combine écriture et vidage
mut interface BufferedOutput extends Output, Flushable {}

mut interface Input {
    mutable Result<Option<string>, IoError> readLine();   // Ok(None) = EOF
    mutable Result<Option<char>, IoError>   readChar();
    mutable Result<string, IoError>         readAll();
}
```

`IoError` est un enum (`BrokenPipe`, `WriteFailed(msg)`, `ReadFailed(msg)`, `Other(msg)`) avec une méthode `message()`.

## Flux standard (`minilang.system`)

`StandardOutput` (stdout) et `StandardError` (stderr) sont des **services injectables** implémentant `BufferedOutput` :

```java
StandardOutput out = inject StandardOutput;
out.writeLine("bonjour");

StandardError err = inject StandardError;
err.writeLine("attention");
```

### Lecture sur l'entrée standard

`StandardInput` (stdin) est aussi un service injectable :

```java
StandardInput in = inject StandardInput;
bool fini = false;
while (!fini) {
    Result<Option<string>, IoError> r = in.readLine();
    match r.getValue() {
        Option::Some(ligne) => { /* traiter la ligne */ }
        Option::None        => { fini = true; }   // EOF
    }
}
```

`readLine()` retire le saut de ligne final ; `readChar()` lit un caractère Unicode ; `readAll()` lit tout le reste. Voir l'exemple [examples/example_stdin.mini](../examples/example_stdin.mini) (à alimenter par un pipe).

## Capture en mémoire et testabilité

`StringOutput` (dans `minilang.io`) est une sortie qui accumule le texte en mémoire (équivalent du `StringWriter` de Java). Comme `Output` est une interface et que les flux sont injectables, on teste du code d'I/O **sans toucher au code testé** : un module de test binde `Output` sur `StringOutput`, puis on relit `content()`.

```java
service class Report {
    Output out;
    Report(Output out) { this.out = out; }
    void render() { out.writeLine("ligne 1"); out.writeLine("ligne 2"); }
}

// En test : on capture au lieu d'écrire sur la console
module TestModule { bind Output to StringOutput; }

test void leRapportEcritDeuxLignes() {
    Report r = inject Report;
    r.render();
    StringOutput captured = inject StringOutput;   // même singleton
    assertEquals(captured.content(), "ligne 1\nligne 2\n");
}
```

Symétriquement, `StringInput` (dans `minilang.io`) est une entrée en mémoire : on l'alimente avec `feed(string)` puis le code testé lit normalement. En test on binde `Input` sur `StringInput`.

`BufferedWriter` (dans `minilang.io`) est une sortie bufferisée concrète : elle enveloppe un `Output` quelconque, accumule les écritures et ne les transmet qu'au `flush()`. À construire explicitement (`new BufferedWriter(target)`).

C'est le bénéfice combiné de l'injection de dépendances, des modules de binding et de l'héritage d'interface.

> Note : `StandardOutput` / `StandardError` / `StandardInput` parlent aux vrais flux du processus ; leur sortie n'est pas capturée par `print`, et lire `StandardInput` bloque en attente d'entrée. En test, on passe par `StringOutput` / `StringInput` (binding).

Voir les exemples : [examples/example_io.mini](../examples/example_io.mini) (sorties) et [examples/example_stdin.mini](../examples/example_stdin.mini) (entrée).
