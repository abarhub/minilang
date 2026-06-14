// ── Qualificateurs d'immutabilité ─────────────────────────────────────────────

// ── Visibilité des membres ────────────────────────────────────────────────────

/// Visibilité d'une méthode de classe ou d'enum.
/// - `Public`    (défaut) : accessible de partout.
/// - `Protected`          : accessible depuis la classe et ses sous-classes.
/// - `Private`            : accessible depuis la classe déclarante uniquement.
///
/// Les champs sont toujours privés — pas de modificateur possible.
/// Les méthodes d'interface sont toujours publiques.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Public,
    Protected,
    Private,
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Visibility::Public    => write!(f, ""),
            Visibility::Protected => write!(f, "protected "),
            Visibility::Private   => write!(f, "private "),
        }
    }
}

// ── Qualificateurs d'immutabilité ─────────────────────────────────────────────

/// Qualificateur d'une variable ou d'un paramètre.
/// - `Mutable`   (défaut) : peut appeler des méthodes `mutable`.
/// - `Readonly`  : vue en lecture seule ; ne peut pas appeler de méthodes `mutable`.
/// - `Immutable` : immuable en profondeur ; peut être passé là où `readonly` est attendu.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Qualifier {
    #[default]
    Mutable,
    Readonly,
    Immutable,
}

impl std::fmt::Display for Qualifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Qualifier::Mutable   => write!(f, ""),
            Qualifier::Readonly  => write!(f, "readonly "),
            Qualifier::Immutable => write!(f, "immutable "),
        }
    }
}

// ── Package & imports ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PackageDecl { pub path: String }

#[derive(Debug, Clone)]
pub struct Import { pub path: String, pub wildcard: bool }

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int, Bool, Str, Char, Float, Double, Void,
    /// Octet non signé (0–255). Type de stockage, sans arithmétique.
    Byte,
    Array(Box<Type>),
    Generic(String, Vec<Type>),
    UserDefined(String),
    /// Lambda non annotée : `fn`  (sentinelle — compatible avec tout)
    Fn,
    /// Lambda typée : `fn(int, string) -> bool`
    FnType(Vec<Type>, Box<Type>),
}

impl Type {
    /// Nom de tête d'un type de référence (`UserDefined` ou `Generic`).
    /// `None` pour les primitifs, arrays, lambdas, etc.
    pub fn ref_name(&self) -> Option<&str> {
        match self {
            Type::UserDefined(n) | Type::Generic(n, _) => Some(n),
            _ => None,
        }
    }

    /// Arguments de type d'une référence générique (vide si non générique).
    pub fn ref_args(&self) -> &[Type] {
        match self {
            Type::Generic(_, a) => a,
            _ => &[],
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Int            => write!(f, "int"),
            Type::Byte           => write!(f, "byte"),
            Type::Bool           => write!(f, "bool"),
            Type::Str            => write!(f, "string"),
            Type::Char           => write!(f, "char"),
            Type::Float          => write!(f, "float"),
            Type::Double         => write!(f, "double"),
            Type::Void           => write!(f, "void"),
            Type::Fn             => write!(f, "fn"),
            Type::Array(i)       => write!(f, "{}[]", i),
            Type::UserDefined(n) => write!(f, "{}", n),
            Type::Generic(n, a)  => {
                let s: Vec<_> = a.iter().map(|t| t.to_string()).collect();
                write!(f, "{}<{}>", n, s.join(", "))
            }
            Type::FnType(params, ret) => {
                let ps: Vec<_> = params.iter().map(|t| t.to_string()).collect();
                write!(f, "fn({}) -> {}", ps.join(", "), ret)
            }
        }
    }
}

// ── Alias de type ─────────────────────────────────────────────────────────────

/// `type Adder = fn(int, int) -> int;`
#[derive(Debug, Clone)]
pub struct TypeAlias { pub name: String, pub ty: Type }

// ── Opérateurs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod, Pow,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinOp::Add => "+",  BinOp::Sub => "-",  BinOp::Mul => "*",
            BinOp::Div => "/",  BinOp::Mod => "%",  BinOp::Pow => "**",
            BinOp::Eq  => "==", BinOp::Ne  => "!=",
            BinOp::Lt  => "<",  BinOp::Le  => "<=",
            BinOp::Gt  => ">",  BinOp::Ge  => ">=",
            BinOp::And => "&&", BinOp::Or  => "||",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp { Neg, Not }

// ── Paramètre ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Param { pub ty: Type, pub name: String }

// ── Corps d'une lambda ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LambdaBody {
    Expr(Box<Expr>),
    Block(Vec<Stmt>),
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i64), FloatLit(f64), BoolLit(bool), StringLit(String), CharLit(char), Ident(String),
    BinOp    { left: Box<Expr>, op: BinOp, right: Box<Expr> },
    UnaryOp  { op: UnaryOp, expr: Box<Expr> },
    FieldAccess  { object: Box<Expr>, field:  String },
    MethodCall   { object: Box<Expr>, method: String, args: Vec<Expr> },
    FunctionCall { name: String, args: Vec<Expr> },
    New             { class_name: String, type_args: Vec<Type>, args: Vec<Expr> },
    /// `inject T` — résolution d'un service par le conteneur d'injection.
    /// T est une classe `service` ou une interface implémentée par exactement
    /// un service. Validé entièrement au typecheck — l'exécution ne peut pas échouer.
    Inject(Type),
    EnumConstructor { enum_name: String, type_args: Vec<Type>, variant: String, args: Vec<Expr> },
    Lambda          { params: Vec<String>, body: LambdaBody },
    LambdaCall      { callee: Box<Expr>, args: Vec<Expr> },
    /// `expr?.field`  — renvoie Option<FieldType>
    SafeFieldAccess { object: Box<Expr>, field: String },
    /// `expr?.method(args)` — renvoie Option<ReturnType>
    SafeMethodCall  { object: Box<Expr>, method: String, args: Vec<Expr> },
    /// `expr ?? default` — déwrappe ou retourne default
    NullCoalesce    { expr: Box<Expr>, default: Box<Expr> },
    /// `new T[]{a, b, ...}` — tableau littéral
    ArrayLit { elem_type: Type, elements: Vec<Expr> },
    /// `new T[n]` — tableau de taille n initialisé à la valeur par défaut
    /// `new T[n](fill)` — tableau de taille n initialisé avec la valeur fill
    ArrayNew { elem_type: Type, size: Box<Expr>, fill: Option<Box<Expr>> },
    /// `obj[idx]` — accès indexé
    Index    { object: Box<Expr>, index: Box<Expr> },
}

// ── Pattern pour match ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Pattern {
    Variant { name: String, bindings: Vec<String> },
    Wildcard,
}

#[derive(Debug, Clone)]
pub struct MatchArm { pub pattern: Pattern, pub body: Vec<Stmt> }

// ── Instructions ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl     { qualifier: Qualifier, ty: Type, name: String, init: Option<Expr> },
    Assign      { target: String, value: Expr },
    FieldAssign { object: String, field: String, value: Expr },
    Print(Vec<Expr>),
    Return(Option<Expr>),
    ExprStmt(Expr),
    If      { condition: Expr, then_body: Vec<Stmt>, else_body: Option<Vec<Stmt>> },
    While   { condition: Expr, body: Vec<Stmt> },
    DoWhile { body: Vec<Stmt>, condition: Expr },
    For     { init: Option<Box<Stmt>>, condition: Option<Expr>,
              update: Option<Box<Stmt>>, body: Vec<Stmt> },
    /// `for (T varName in expr) { body }` — syntaxe d'itération sur Iterable<T>
    ForIn   { var_type: Type, var_name: String, iter_expr: Box<Expr>, body: Vec<Stmt> },
    Break, Continue,
    Match { expr: Expr, arms: Vec<MatchArm> },
    /// Méthode native — corps de tableau (no-op à l'exécution)
    Builtin,
}

// ── Membres de classe ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Field  { pub ty: Type, pub name: String }

#[derive(Debug, Clone)]
pub struct Method {
    pub visibility:  Visibility,
    pub is_mutable:  bool,
    pub return_type: Type, pub name: String,
    pub params: Vec<Param>, pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Constructor { pub params: Vec<Param>, pub body: Vec<Stmt> }

#[derive(Debug, Clone)]
pub struct MethodSig { pub is_mutable: bool, pub return_type: Type, pub name: String, pub params: Vec<Param> }

// ── Classe ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClassDef {
    /// true si la classe est déclarée `service` — instanciable par le conteneur
    /// d'injection de dépendances (singleton, dépendances via le constructeur).
    pub is_service: bool,
    /// true si le service est déclaré `transient` — nouvelle instance à chaque
    /// injection au lieu d'un singleton. Nécessite `service`.
    pub is_transient: bool,
    pub is_mut:   bool,
    pub name: String, pub type_params: Vec<String>,
    /// Contraintes de qualificateur sur les paramètres de type.
    /// Ex. : `mut class Map<immutable K, V>` → `[("K", Immutable)]`
    pub type_param_constraints: Vec<(String, Qualifier)>,
    /// Classe parente (`extends Base<int>`). `Type::UserDefined` ou `Type::Generic`.
    pub parent: Option<Type>,
    /// Interfaces implémentées (`implements Box<int>`). Args de type conservés.
    pub implements: Vec<Type>,
    pub fields: Vec<Field>, pub constructors: Vec<Constructor>, pub methods: Vec<Method>,
}

// ── Interface ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InterfaceDef {
    pub is_mut: bool,
    pub name: String, pub type_params: Vec<String>,
    pub type_param_constraints: Vec<(String, Qualifier)>,
    /// Interfaces étendues (`interface Sub extends A<int>, B`). Une classe qui
    /// implémente Sub doit fournir les méthodes de Sub et de ses parents.
    /// Les args de type sur le parent sont conservés (`Type::Generic`/`UserDefined`).
    pub parents: Vec<Type>,
    pub methods: Vec<MethodSig>,
}

// ── Enum ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EnumVariant { pub name: String, pub fields: Vec<Param> }

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String, pub type_params: Vec<String>,
    pub type_param_constraints: Vec<(String, Qualifier)>,
    pub implements: Vec<String>,
    pub variants: Vec<EnumVariant>, pub methods: Vec<Method>,
}

// ── Record ────────────────────────────────────────────────────────────────────

/// Agrégat de données immuable.
///
/// `record Point(int x, int y) {}`
///
/// - Champs toujours privés et immuables (pas de méthode `mutable` autorisée).
/// - Hérite implicitement de la classe abstraite `Record`.
/// - Peut implémenter des interfaces.
/// - Génère automatiquement : getters, `equals`, `toString`, `hashCode`, `copy`.
#[derive(Debug, Clone)]
pub struct RecordDef {
    pub name:                   String,
    pub type_params:            Vec<String>,
    pub type_param_constraints: Vec<(String, Qualifier)>,
    pub fields:                 Vec<Field>,   // ordonnés — champs du record
    pub methods:                Vec<Method>,  // méthodes custom non-mutable
    pub implements:             Vec<String>,
}

// ── Module d'injection de dépendances ─────────────────────────────────────────

/// Une directive de binding dans un module :
/// - `bind Iface to Service;`              — choisit l'implémentation d'une interface
/// - `bind Service with (val, ...);`       — fournit les paramètres de configuration
/// - `bind Iface to Service with (val, ...);` — les deux
#[derive(Debug, Clone)]
pub struct BindDecl {
    pub target: String,
    pub to:     Option<String>,
    pub with:   Vec<Expr>,
}

/// `module AppModule { bind ...; }` — configuration centralisée du conteneur
/// d'injection. Plusieurs modules peuvent coexister ; leurs bindings sont
/// fusionnés (un binding dupliqué est une erreur de compilation).
#[derive(Debug, Clone)]
pub struct ModuleDef { pub name: String, pub binds: Vec<BindDecl> }

// ── Fonction de haut niveau ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FuncDef {
    /// true si la fonction est déclarée `test` — exécutée par le runner de
    /// tests (`mini_parser test`). Doit être `void` et sans paramètres.
    pub is_test:     bool,
    pub return_type: Type,
    pub name:        String,
    pub params:      Vec<Param>,
    pub body:        Vec<Stmt>,
}

// ── Fonction main ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MainFunc { pub body: Vec<Stmt> }

// ── Programme complet ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Program {
    pub package:      Option<PackageDecl>,
    pub imports:      Vec<Import>,
    pub type_aliases: Vec<TypeAlias>,
    pub modules:      Vec<ModuleDef>,
    pub interfaces:   Vec<InterfaceDef>,
    pub enums:        Vec<EnumDef>,
    pub records:      Vec<RecordDef>,
    pub classes:      Vec<ClassDef>,
    pub funcs:        Vec<FuncDef>,
    /// Optionnel : un fichier de tests peut ne pas avoir de main.
    /// L'exécution normale (mode run) exige sa présence.
    pub main:         Option<MainFunc>,
}
