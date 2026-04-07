// ── Package & imports ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PackageDecl { pub path: String }

#[derive(Debug, Clone)]
pub struct Import { pub path: String, pub wildcard: bool }

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int, Bool, Str, Float, Double, Void,
    Array(Box<Type>),
    Generic(String, Vec<Type>),
    UserDefined(String),
    /// Type d'une valeur lambda (fermeture)
    Fn,
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
            Type::Fn             => write!(f, "fn"),
            Type::Array(i)       => write!(f, "{}[]", i),
            Type::UserDefined(n) => write!(f, "{}", n),
            Type::Generic(n, a)  => {
                let s: Vec<_> = a.iter().map(|t| t.to_string()).collect();
                write!(f, "{}<{}>", n, s.join(", "))
            }
        }
    }
}

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
    /// `x => x * 2`  — expression, valeur implicitement retournée
    Expr(Box<Expr>),
    /// `(x, y) => { int z = x + y; return z; }`  — bloc complet
    Block(Vec<Stmt>),
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i64), FloatLit(f64), BoolLit(bool), StringLit(String), Ident(String),
    BinOp    { left: Box<Expr>, op: BinOp, right: Box<Expr> },
    UnaryOp  { op: UnaryOp, expr: Box<Expr> },
    FieldAccess  { object: Box<Expr>, field:  String },
    MethodCall   { object: Box<Expr>, method: String, args: Vec<Expr> },
    FunctionCall { name: String, args: Vec<Expr> },
    New              { class_name: String, type_args: Vec<Type>, args: Vec<Expr> },
    EnumConstructor  { enum_name: String, variant: String, args: Vec<Expr> },
    /// `x => x + 1`  ou  `(x, y) => { return x + y; }`
    Lambda { params: Vec<String>, body: LambdaBody },
    /// Appel d'une lambda stockée dans une variable : `f(1, 2)`
    LambdaCall { callee: Box<Expr>, args: Vec<Expr> },
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
    VarDecl     { ty: Type, name: String, init: Option<Expr> },
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
    Break, Continue,
    Match { expr: Expr, arms: Vec<MatchArm> },
}

// ── Membres de classe ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Field  { pub ty: Type, pub name: String }

#[derive(Debug, Clone)]
pub struct Method {
    pub return_type: Type,
    pub name:        String,
    pub params:      Vec<Param>,
    pub body:        Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Constructor { pub params: Vec<Param>, pub body: Vec<Stmt> }

#[derive(Debug, Clone)]
pub struct MethodSig { pub return_type: Type, pub name: String, pub params: Vec<Param> }

// ── Définition de classe ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name:         String,
    pub type_params:  Vec<String>,
    pub parent:       Option<String>,
    pub implements:   Vec<String>,
    pub fields:       Vec<Field>,
    pub constructors: Vec<Constructor>,
    pub methods:      Vec<Method>,
}

// ── Définition d'interface ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InterfaceDef { pub name: String, pub methods: Vec<MethodSig> }

// ── Enum ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EnumVariant { pub name: String, pub fields: Vec<Param> }

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name:     String,
    pub variants: Vec<EnumVariant>,
    pub methods:  Vec<Method>,
}

// ── Fonction main ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MainFunc { pub body: Vec<Stmt> }

// ── Programme complet ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Program {
    pub package:    Option<PackageDecl>,
    pub imports:    Vec<Import>,
    pub interfaces: Vec<InterfaceDef>,
    pub enums:      Vec<EnumDef>,
    pub classes:    Vec<ClassDef>,
    pub main:       MainFunc,
}
