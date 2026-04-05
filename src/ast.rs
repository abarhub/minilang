// ─────────────────────────────────────────────
//  AST – nœuds de l'arbre syntaxique abstrait
// ─────────────────────────────────────────────

// ── Déclaration de package & imports ─────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PackageDecl {
    pub path: String, // "com.example.myapp"
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path:     String, // "com.example.Foo"  ou  "com.example.*"
    pub wildcard: bool,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Bool,
    Str,
    Float,
    Double,
    Void,
    Array(Box<Type>),
    /// Type générique instancié : Stack<int>, Pair<string, int>
    Generic(String, Vec<Type>),
    /// Identifiant simple : classe utilisateur ou paramètre de type
    UserDefined(String),
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Int            => write!(f, "int"),
            Type::Bool           => write!(f, "bool"),
            Type::Str            => write!(f, "string"),
            Type::Float          => write!(f, "float"),
            Type::Double         => write!(f, "double"),
            Type::Void           => write!(f, "void"),
            Type::Array(inner)   => write!(f, "{}[]", inner),
            Type::UserDefined(n) => write!(f, "{}", n),
            Type::Generic(n, args) => {
                let a: Vec<String> = args.iter().map(|t| t.to_string()).collect();
                write!(f, "{}<{}>", n, a.join(", "))
            }
        }
    }
}

// ── Opérateurs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    // arithmétique
    Add, Sub, Mul, Div, Mod, Pow,
    // comparaison
    Eq, Ne, Lt, Le, Gt, Ge,
    // logique
    And, Or,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*",
            BinOp::Div => "/", BinOp::Mod => "%", BinOp::Pow => "**",
            BinOp::Eq  => "==", BinOp::Ne => "!=",
            BinOp::Lt  => "<",  BinOp::Le => "<=",
            BinOp::Gt  => ">",  BinOp::Ge => ">=",
            BinOp::And => "&&", BinOp::Or => "||",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp { Neg, Not }

// ── Paramètre ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Param {
    pub ty:   Type,
    pub name: String,
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    StringLit(String),
    Ident(String),

    BinOp {
        left:  Box<Expr>,
        op:    BinOp,
        right: Box<Expr>,
    },
    UnaryOp {
        op:   UnaryOp,
        expr: Box<Expr>,
    },

    FieldAccess {
        object: Box<Expr>,
        field:  String,
    },
    MethodCall {
        object: Box<Expr>,
        method: String,
        args:   Vec<Expr>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },

    /// new ClassName<TypeArgs>(args)
    New {
        class_name: String,
        type_args:  Vec<Type>,
        args:       Vec<Expr>,
    },
}

// ── Instructions ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl {
        ty:   Type,
        name: String,
        init: Option<Expr>,
    },
    Assign {
        target: String,
        value:  Expr,
    },
    FieldAssign {
        object: String,
        field:  String,
        value:  Expr,
    },
    Print(Vec<Expr>),
    Return(Option<Expr>),
    ExprStmt(Expr),

    // ── Contrôle de flux ─────────────────────────────────────────────────────
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body:      Vec<Stmt>,
    },
    DoWhile {
        body:      Vec<Stmt>,
        condition: Expr,
    },
    For {
        init:      Option<Box<Stmt>>,
        condition: Option<Expr>,
        update:    Option<Box<Stmt>>,
        body:      Vec<Stmt>,
    },
    Break,
    Continue,
}

// ── Membres de classe ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Field {
    pub ty:   Type,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Method {
    pub return_type: Type,
    pub name:        String,
    pub params:      Vec<Param>,
    pub body:        Vec<Stmt>,
}

/// Constructeur : même nom que la classe, pas de type de retour
#[derive(Debug, Clone)]
pub struct Constructor {
    pub params: Vec<Param>,
    pub body:   Vec<Stmt>,
}

// ── Signature de méthode (pour les interfaces) ────────────────────────────────

#[derive(Debug, Clone)]
pub struct MethodSig {
    pub return_type: Type,
    pub name:        String,
    pub params:      Vec<Param>,
}

// ── Définition de classe ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name:         String,
    /// Paramètres de type : ["T", "K"] pour class Foo<T, K>
    pub type_params:  Vec<String>,
    pub parent:       Option<String>,
    pub implements:   Vec<String>,
    pub fields:       Vec<Field>,
    pub constructors: Vec<Constructor>,
    pub methods:      Vec<Method>,
}

// ── Définition d'interface ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InterfaceDef {
    pub name:    String,
    pub methods: Vec<MethodSig>,
}

// ── Fonction main ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MainFunc {
    pub body: Vec<Stmt>,
}

// ── Programme complet ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Program {
    pub package:    Option<PackageDecl>,
    pub imports:    Vec<Import>,
    pub interfaces: Vec<InterfaceDef>,
    pub classes:    Vec<ClassDef>,
    pub main:       MainFunc,
}
