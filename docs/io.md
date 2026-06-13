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

`byte[]` est un tableau ordinaire (`new byte[n]`, `get`/`set`…), distinct de `int[]`. Il n'y a **pas** de flux binaire : on lit/écrit les octets en masse (voir `Files` ci-dessous) et on convertit vers/depuis `string` avec `Bytes`.

## Fichiers : la classe `Files`

`Files` (injectable, `minilang.io`) lit et écrit des fichiers **en bloc** — pas de flux, pas de handle à fermer (donc aucune fuite de ressource possible). Modèle cohérent avec le reste : les **octets sont la donnée primitive**, la string en est un décodage UTF-8 faillible.

```java
Files files = inject Files;

// Texte (UTF-8)
files.writeText("notes.txt", "bonjour\n");
files.appendText("notes.txt", "monde\n");
Result<string, IoError> txt = files.readText("notes.txt");   // Err si UTF-8 invalide

// Binaire (byte[])
Result<byte[], IoError> raw = files.readBytes("photo.png");
files.writeBytes("copie.png", raw.getValue());

// Divers
bool present = files.exists("notes.txt");
files.delete("notes.txt");
```

| Méthode | Retour |
|---|---|
| `readBytes(path)` / `readText(path)` | `Result<byte[], IoError>` / `Result<string, IoError>` |
| `writeBytes(path, data)` / `writeText(path, s)` | `Result<Unit, IoError>` (écrase) |
| `appendBytes(path, data)` / `appendText(path, s)` | `Result<Unit, IoError>` (crée si absent) |
| `exists(path)` | `bool` |
| `delete(path)` | `Result<Unit, IoError>` |

> Note : `Files` travaille sur des chemins **bruts**, sans garde-fou. Pour un accès **confiné**, préférer les capacités de répertoire (ci-dessous). Le streaming de fichiers (ligne par ligne via `Input`/`Output`) n'est pas fourni.

## Accès confiné : les capacités de répertoire

Plutôt que des chemins bruts, on peut travailler avec des **capacités** : un objet qui représente l'autorité d'accéder à un sous-arbre, et rien au-dessus. C'est le modèle des *object-capabilities* (cf. preopens de WASI, Capsicum).

- On n'écrit jamais de chemin absolu : on obtient une racine via `FileSystem` (seul à pouvoir en créer une — pas d'autorité ambiante), puis on accède à des **enfants relatifs**.
- On ne peut que **restreindre** : `sub`/`subRW` dérivent une capacité sur un sous-répertoire, jamais au-dessus. `..` et les chemins absolus sont rejetés.
- Le **mode est dans le type** : `ReadDir` (lecture) vs `ReadWriteDir extends ReadDir` (+ écriture). Une fonction qui reçoit un `ReadDir` ne peut **pas compiler** une écriture — la restriction de droits est garantie à la compilation.

```java
FileSystem fs = inject FileSystem;
ReadWriteDir root = fs.tempDir().getValue();     // racine temporaire (une par exécution)

root.writeText("notes.txt", "bonjour");
ReadWriteDir conf = root.subRW("config");        // sous-répertoire RW (parents créés à l'écriture)
conf.writeText("app.ini", "mode=demo");

ReadDir ro = root.sub("config");                 // vue lecture seule
// ro.writeText(...)  →  ne compile pas

root.readText("../secret");                       // → Err : hors de la capacité
```

| Type | Méthodes |
|---|---|
| `ReadDir` | `readBytes`/`readText`, `exists`, `sub` (→ `ReadDir`), `name` |
| `ReadWriteDir` (extends `ReadDir`) | + `writeBytes`/`writeText`, `appendBytes`/`appendText`, `delete`, `subRW` (→ `ReadWriteDir`) |

### Sources de racine

- **`FileSystem.tempDir()`** → `Result<ReadWriteDir, IoError>` : un répertoire temporaire frais (un par appel).
- **`FileSystem.root(nom)` / `rootRW(nom)`** : une racine **nommée** configurée dans le `minilang.toml` (`[files.roots]`). `root` donne une vue lecture seule ; `rootRW` échoue si la racine est en lecture seule ou inconnue.

```toml
[files.roots.data]
path = "data"            # relatif au minilang.toml
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

Les répertoires configurés doivent **exister au démarrage** (sinon erreur de configuration fatale) ; leur chemin est canonicalisé. C'est la config — hors du programme — qui octroie les racines : le code ne peut pas en forger une.

Encore à venir : un garde-fou contre les symlinks, et le nettoyage des répertoires temporaires (marqueur `.minilang-temp` pour un processus externe). Modèle de menace actuel : prévention des évasions accidentelles et code coopératif — pas la défense contre un programme qui planterait un lien symbolique.

Voir l'exemple : [examples/example_file_capabilities.mini](../examples/example_file_capabilities.mini).

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
