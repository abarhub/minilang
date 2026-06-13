// ─────────────────────────────────────────────────────────────────────────────
//  Interpréteur
// ─────────────────────────────────────────────────────────────────────────────

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use log::{debug, info, warn};

use crate::ast::*;

// ── Valeurs runtime ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ObjectData { pub class_name: String, pub fields: HashMap<String, Value> }

#[derive(Debug, Clone)]
pub struct EnumData {
    pub enum_name:    String,
    pub variant_name: String,
    pub fields:       HashMap<String, Value>,
    pub field_order:  Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64), Byte(u8), Float(f64), Bool(bool), Str(String), Char(char),
    Array(Rc<RefCell<Vec<Value>>>),
    HashMap(Rc<RefCell<Vec<(Value, Value)>>>),
    Object(Rc<RefCell<ObjectData>>),
    Enum(Rc<EnumData>),
    /// Fermeture : paramètres nommés, corps, variables capturées au moment de la création
    Lambda { params: Vec<String>, body: LambdaBody, captured: HashMap<String, Value> },
    Null, Void,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(n)    => write!(f, "{}", n),
            Value::Byte(b)   => write!(f, "{}", b),
            Value::Float(n)  => write!(f, "{}", n),
            Value::Bool(b)   => write!(f, "{}", b),
            Value::Str(s)    => write!(f, "{}", s),
            Value::Char(c)   => write!(f, "{}", c),
            Value::Null      => write!(f, "null"),
            Value::Void      => write!(f, ""),
            Value::Array(v)  => {
                write!(f, "[{}]", v.borrow().iter()
                    .map(|x| x.to_string()).collect::<Vec<_>>().join(", "))
            }
            Value::HashMap(v) => {
                write!(f, "HashMap{{{}}}", v.borrow().iter()
                    .map(|(k, val)| format!("{}={}", k, val))
                    .collect::<Vec<_>>().join(", "))
            }
            Value::Object(o) => write!(f, "<{}>", o.borrow().class_name),
            Value::Enum(e)   => {
                if e.field_order.is_empty() {
                    write!(f, "{}::{}", e.enum_name, e.variant_name)
                } else {
                    let vals: Vec<_> = e.field_order.iter().map(|k| e.fields[k].to_string()).collect();
                    write!(f, "{}::{}({})", e.enum_name, e.variant_name, vals.join(", "))
                }
            }
            Value::Lambda { params, .. } => write!(f, "<fn({})>", params.join(", ")),
        }
    }
}

// ── Erreur runtime ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct RuntimeError(pub String);

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RuntimeError: {}", self.0)
    }
}

macro_rules! err { ($($a:tt)*) => { Err(RuntimeError(format!($($a)*))) }; }

// ── Environnement (pile de scopes) ────────────────────────────────────────────

pub struct Env { scopes: Vec<HashMap<String, Value>> }

impl Env {
    pub fn new() -> Self { Self { scopes: vec![HashMap::new()] } }
    pub fn push(&mut self) { self.scopes.push(HashMap::new()); }
    pub fn pop(&mut self)  { if self.scopes.len() > 1 { self.scopes.pop(); } }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }
    pub fn set(&mut self, name: String, val: Value) {
        for s in self.scopes.iter_mut().rev() {
            if s.contains_key(&name) { s.insert(name, val); return; }
        }
        self.scopes.last_mut().unwrap().insert(name, val);
    }
    pub fn declare(&mut self, name: String, val: Value) {
        self.scopes.last_mut().unwrap().insert(name, val);
    }

    /// Capture un snapshot de tout l'environnement courant (pour les fermetures)
    pub fn snapshot(&self) -> HashMap<String, Value> {
        let mut snap = HashMap::new();
        for scope in &self.scopes {
            for (k, v) in scope { snap.insert(k.clone(), v.clone()); }
        }
        snap
    }
}

// ── Flux de contrôle ─────────────────────────────────────────────────────────

#[derive(Debug)]
enum Flow { Next, Break, Continue, Return(Value) }

// ── Interpréteur ─────────────────────────────────────────────────────────────

pub struct Interpreter {
    classes:  HashMap<String, ClassDef>,
    enums:    HashMap<String, EnumDef>,
    funcs:    HashMap<String, FuncDef>,
    /// Noms des interfaces — pour classifier les paramètres de constructeur
    /// des services (dépendance vs valeur de configuration)
    interfaces: std::collections::HashSet<String>,
    /// Interface → ses interfaces parentes (pour la conformité transitive d'un
    /// service à une interface lors de la résolution d'injection)
    iface_parents: HashMap<String, Vec<String>>,
    /// Bindings explicites des modules : interface → service concret
    binds_to: HashMap<String, String>,
    /// Valeurs de configuration des modules : service → args du `with`
    with_values: HashMap<String, Vec<Expr>>,
    /// Instances des services déjà créées par `inject` (nom de classe → singleton)
    singletons: HashMap<String, Value>,
    print_fn: Box<dyn FnMut(&str)>,
}

impl Interpreter {
    /// Crée un interpréteur qui affiche sur la console (comportement par défaut).
    pub fn new(program: &Program) -> Self {
        Self::new_with_print(program, Box::new(|line| println!("{}", line)))
    }

    /// Crée un interpréteur avec une fonction d'affichage personnalisée.
    pub fn new_with_print(program: &Program, print_fn: Box<dyn FnMut(&str)>) -> Self {
        // Records en premier, puis classes — les classes utilisateur ont priorité
        // (permet à un fichier de redéfinir une classe stdlib même si la stdlib
        // expose un record du même nom, comme Pair).
        let mut classes: HashMap<String, ClassDef> = program.records.iter()
            .map(|rd| (rd.name.clone(), crate::typechecker::TypeChecker::record_to_class_pub(rd)))
            .collect();
        for c in &program.classes {
            classes.insert(c.name.clone(), c.clone());
        }
        // Bindings et valeurs de configuration des modules — la validation
        // (cibles connues, doublons, types) a déjà été faite par le typechecker.
        let mut binds_to:    HashMap<String, String>    = HashMap::new();
        let mut with_values: HashMap<String, Vec<Expr>> = HashMap::new();
        for m in &program.modules {
            for b in &m.binds {
                let concrete = b.to.clone().unwrap_or_else(|| b.target.clone());
                if b.to.is_some() { binds_to.insert(b.target.clone(), concrete.clone()); }
                if !b.with.is_empty() { with_values.insert(concrete, b.with.clone()); }
            }
        }
        Self {
            classes,
            enums:    program.enums  .iter().map(|e| (e.name.clone(), e.clone())).collect(),
            funcs:    program.funcs  .iter().map(|f| (f.name.clone(), f.clone())).collect(),
            interfaces: program.interfaces.iter().map(|i| i.name.clone()).collect(),
            iface_parents: program.interfaces.iter()
                .map(|i| (i.name.clone(), i.parents.clone())).collect(),
            binds_to,
            with_values,
            singletons: HashMap::new(),
            print_fn,
        }
    }

    pub fn run(&mut self, program: &Program) -> Result<i64, RuntimeError> {
        info!("▶ Exécution");
        let Some(main) = &program.main else {
            return err!("Aucune fonction main — rien à exécuter \
                         (utilisez `mini_parser test` pour un fichier de tests)");
        };
        let mut env = Env::new();
        match self.exec_body(&main.body.clone(), &mut env, None)? {
            Flow::Return(Value::Int(n)) => { info!("✓ main → {}", n); Ok(n) }
            Flow::Return(v) => { warn!("main valeur non-int : {}", v); Ok(0) }
            _ => { warn!("main sans return"); Ok(0) }
        }
    }

    /// Exécute une fonction de test (sans paramètres) dans un environnement
    /// neuf. Utilisé par le runner de tests — chaque test tourne dans un
    /// interpréteur fraîchement créé (singletons DI réinitialisés).
    pub fn run_test(&mut self, name: &str) -> Result<(), RuntimeError> {
        let func = self.funcs.get(name).cloned()
            .ok_or_else(|| RuntimeError(format!("Fonction de test inconnue '{}'", name)))?;
        let mut env = Env::new();
        self.exec_body(&func.body, &mut env, None)?;
        Ok(())
    }

    // ── Valeur par défaut ─────────────────────────────────────────────────────

    fn default_value(ty: &Type) -> Value {
        match ty {
            Type::Int            => Value::Int(0),
            Type::Byte           => Value::Byte(0),
            Type::Bool           => Value::Bool(false),
            Type::Float | Type::Double => Value::Float(0.0),
            Type::Str            => Value::Str(String::new()),
            Type::Char           => Value::Char('\0'),
            Type::Array(_)       => Value::Array(Rc::new(RefCell::new(vec![]))),
            _                    => Value::Null,
        }
    }

    // ── Instanciation ─────────────────────────────────────────────────────────

    fn instantiate(&self, class_name: &str) -> Result<Value, RuntimeError> {
        if !self.classes.contains_key(class_name) {
            return err!("Classe inconnue : '{}'", class_name);
        }
        let fields = self.all_fields(class_name).iter()
            .map(|f| (f.name.clone(), Self::default_value(&f.ty))).collect();
        Ok(Value::Object(Rc::new(RefCell::new(ObjectData {
            class_name: class_name.to_string(), fields
        }))))
    }

    fn all_fields(&self, cn: &str) -> Vec<Field> {
        let Some(c) = self.classes.get(cn) else { return vec![]; };
        let mut fs = c.parent.as_deref().map(|p| self.all_fields(p)).unwrap_or_default();
        for f in &c.fields { fs.retain(|pf: &Field| pf.name != f.name); fs.push(f.clone()); }
        fs
    }

    fn find_method(&self, cn: &str, mn: &str) -> Option<Method> {
        if let Some(c) = self.classes.get(cn) {
            if let Some(m) = c.methods.iter().find(|m| m.name == mn) { return Some(m.clone()); }
            if let Some(p) = &c.parent { return self.find_method(p, mn); }
            // Fallback vers Object si pas de parent explicite
            if cn != "Object" { return self.find_method("Object", mn); }
        }
        if let Some(e) = self.enums.get(cn) {
            if let Some(m) = e.methods.iter().find(|m| m.name == mn) { return Some(m.clone()); }
        }
        None
    }

    // ── Corps ─────────────────────────────────────────────────────────────────

    fn exec_body(
        &mut self, stmts: &[Stmt], env: &mut Env,
        this: Option<Rc<RefCell<ObjectData>>>,
    ) -> Result<Flow, RuntimeError> {
        for s in stmts {
            match self.exec_stmt(s, env, this.clone())? {
                Flow::Next => {}
                other      => return Ok(other),
            }
        }
        Ok(Flow::Next)
    }

    // ── Instructions ──────────────────────────────────────────────────────────

    fn exec_stmt(
        &mut self, stmt: &Stmt, env: &mut Env,
        this: Option<Rc<RefCell<ObjectData>>>,
    ) -> Result<Flow, RuntimeError> {
        match stmt {
            Stmt::VarDecl { qualifier: _, ty, name, init } => {
                let val = if let Some(e) = init {
                    self.eval(e, env, this)?
                } else if let Type::UserDefined(cn) = ty {
                    if self.enums.contains_key(cn) { Value::Null }
                    else { self.instantiate(cn)? }
                } else if let Type::Generic(cn, _) = ty {
                    self.instantiate(cn)?
                } else {
                    Self::default_value(ty)
                };
                env.declare(name.clone(), val);
            }

            Stmt::Assign { target, value } => {
                let val = self.eval(value, env, this.clone())?;
                let is_field = this.as_ref()
                    .map(|t| t.borrow().fields.contains_key(target)).unwrap_or(false);
                if is_field { this.unwrap().borrow_mut().fields.insert(target.clone(), val); }
                else { env.set(target.clone(), val); }
            }

            Stmt::FieldAssign { object, field, value } => {
                let val = self.eval(value, env, this.clone())?;
                let rc = if object == "this" {
                    this.clone().ok_or_else(|| RuntimeError("'this' hors méthode".into()))?
                } else {
                    let v = env.get(object)
                        .or_else(|| this.as_ref().and_then(|t| t.borrow().fields.get(object).cloned()))
                        .ok_or_else(|| RuntimeError(format!("Variable inconnue '{}'", object)))?;
                    match v { Value::Object(rc) => rc, _ => return err!("'{}' non-objet", object) }
                };
                rc.borrow_mut().fields.insert(field.clone(), val);
            }

            Stmt::Print(args) => {
                let parts: Vec<String> = args.iter()
                    .map(|e| self.eval(e, env, this.clone()).map(|v| v.to_string()))
                    .collect::<Result<_, _>>()?;
                (self.print_fn)(&parts.join(" "));
            }

            Stmt::Return(e) => {
                let v = match e { Some(e) => self.eval(e, env, this)?, None => Value::Void };
                return Ok(Flow::Return(v));
            }

            Stmt::ExprStmt(e) => { self.eval(e, env, this)?; }

            Stmt::If { condition, then_body, else_body } => {
                match self.eval(condition, env, this.clone())? {
                    Value::Bool(true) => {
                        env.push();
                        let f = self.exec_body(then_body, env, this)?;
                        env.pop();
                        if !matches!(f, Flow::Next) { return Ok(f); }
                    }
                    Value::Bool(false) => {
                        if let Some(eb) = else_body {
                            env.push();
                            let f = self.exec_body(eb, env, this)?;
                            env.pop();
                            if !matches!(f, Flow::Next) { return Ok(f); }
                        }
                    }
                    _ => return err!("Condition if non-bool"),
                }
            }

            Stmt::While { condition, body } => loop {
                match self.eval(condition, env, this.clone())? {
                    Value::Bool(false) => break,
                    Value::Bool(true)  => {}
                    _ => return err!("Condition while non-bool"),
                }
                env.push();
                let f = self.exec_body(body, env, this.clone())?;
                env.pop();
                match f {
                    Flow::Break    => break,
                    Flow::Continue => continue,
                    Flow::Return(v) => return Ok(Flow::Return(v)),
                    Flow::Next     => {}
                }
            },

            Stmt::DoWhile { body, condition } => loop {
                env.push();
                let f = self.exec_body(body, env, this.clone())?;
                env.pop();
                match f {
                    Flow::Break    => break,
                    Flow::Return(v) => return Ok(Flow::Return(v)),
                    Flow::Continue | Flow::Next => {}
                }
                match self.eval(condition, env, this.clone())? {
                    Value::Bool(false) => break,
                    Value::Bool(true)  => {}
                    _ => return err!("Condition do-while non-bool"),
                }
            },

            Stmt::ForIn { var_name, iter_expr, body, .. } => {
                let iterable = self.eval(iter_expr, env, this.clone())?;
                match iterable {
                    // ── Tableau natif : itération directe ──────────────────────
                    Value::Array(v) => {
                        let items: Vec<Value> = v.borrow().clone();
                        'arr: for item in items {
                            env.push();
                            env.declare(var_name.clone(), item);
                            let f = self.exec_body(body, env, this.clone())?;
                            env.pop();
                            match f {
                                Flow::Break         => break 'arr,
                                Flow::Return(v)     => return Ok(Flow::Return(v)),
                                Flow::Continue | Flow::Next => {}
                            }
                        }
                    }
                    // ── Objet : appel de iterator() puis next() en boucle ──────
                    Value::Object(rc) => {
                        let cn = rc.borrow().class_name.clone();
                        let iter_m = self.find_method(&cn, "iterator")
                            .ok_or_else(|| RuntimeError(format!(
                                "'{}' n'est pas Iterable : méthode iterator() absente", cn)))?;
                        let iterator = self.call_method(&iter_m, vec![], rc)?;
                        let iter_rc = match iterator {
                            Value::Object(r) => r,
                            _ => return err!("iterator() doit retourner un objet"),
                        };
                        'obj: loop {
                            let iter_cn = iter_rc.borrow().class_name.clone();
                            let next_m = self.find_method(&iter_cn, "next")
                                .ok_or_else(|| RuntimeError(format!(
                                    "Iterator '{}' n'a pas de méthode next()", iter_cn)))?;
                            let next_val = self.call_method(&next_m, vec![], iter_rc.clone())?;
                            match next_val {
                                Value::Enum(ref ed)
                                    if ed.enum_name == "Option" && ed.variant_name == "None"
                                    => break 'obj,
                                Value::Enum(ref ed)
                                    if ed.enum_name == "Option" && ed.variant_name == "Some"
                                    => {
                                    let item = ed.fields.get("value")
                                        .cloned().unwrap_or(Value::Null);
                                    env.push();
                                    env.declare(var_name.clone(), item);
                                    let f = self.exec_body(body, env, this.clone())?;
                                    env.pop();
                                    match f {
                                        Flow::Break     => break 'obj,
                                        Flow::Return(v) => return Ok(Flow::Return(v)),
                                        Flow::Continue | Flow::Next => {}
                                    }
                                }
                                _ => return err!("next() doit retourner Option<T>"),
                            }
                        }
                    }
                    other => return err!("for-in : valeur non itérable : {}", other),
                }
            }

            Stmt::For { init, condition, update, body } => {
                env.push();
                if let Some(s) = init { self.exec_stmt(s, env, this.clone())?; }
                'lp: loop {
                    if let Some(ce) = condition {
                        match self.eval(ce, env, this.clone())? {
                            Value::Bool(false) => break,
                            Value::Bool(true)  => {}
                            _ => return err!("Condition for non-bool"),
                        }
                    }
                    env.push();
                    let f = self.exec_body(body, env, this.clone())?;
                    env.pop();
                    match f {
                        Flow::Break    => break 'lp,
                        Flow::Return(v) => { env.pop(); return Ok(Flow::Return(v)); }
                        Flow::Continue | Flow::Next => {}
                    }
                    if let Some(u) = update { self.exec_stmt(u, env, this.clone())?; }
                }
                env.pop();
            }

            Stmt::Builtin => { /* méthode native — ne devrait pas être exécutée directement */ }

            Stmt::Break    => return Ok(Flow::Break),
            Stmt::Continue => return Ok(Flow::Continue),

            Stmt::Match { expr, arms } => {
                let val = self.eval(expr, env, this.clone())?;
                for arm in arms {
                    let matched = match &arm.pattern {
                        Pattern::Wildcard => true,
                        Pattern::Variant { name, bindings } => match &val {
                            Value::Enum(ed) if ed.variant_name == *name => {
                                env.push();
                                for (b, f) in bindings.iter().zip(ed.field_order.iter()) {
                                    env.declare(b.clone(), ed.fields[f].clone());
                                }
                                true
                            }
                            _ => false,
                        },
                    };
                    if matched {
                        let need_scope = !matches!(&arm.pattern, Pattern::Variant { .. } if matches!(&val, Value::Enum(_)));
                        if need_scope { env.push(); }
                        let f = self.exec_body(&arm.body, env, this.clone())?;
                        env.pop();
                        if !matches!(f, Flow::Next) { return Ok(f); }
                        break;
                    }
                }
            }
        }
        Ok(Flow::Next)
    }

    // ── Évaluation d'expression ───────────────────────────────────────────────

    fn eval(
        &mut self, expr: &Expr, env: &mut Env,
        this: Option<Rc<RefCell<ObjectData>>>,
    ) -> Result<Value, RuntimeError> {
        match expr {
            Expr::IntLit(n)    => Ok(Value::Int(*n)),
            Expr::FloatLit(f)  => Ok(Value::Float(*f)),
            Expr::BoolLit(b)   => Ok(Value::Bool(*b)),
            Expr::StringLit(s) => Ok(Value::Str(s.clone())),
            Expr::CharLit(c)   => Ok(Value::Char(*c)),

            Expr::Ident(name) => {
                // env en premier : permet aux méthodes d'enum de stocker `this = Value::Enum`
                if let Some(v) = env.get(name) { return Ok(v); }
                if name == "this" {
                    return this.as_ref()
                        .map(|rc| Value::Object(rc.clone()))
                        .ok_or_else(|| RuntimeError("'this' hors méthode".into()));
                }
                if let Some(obj) = &this {
                    if let Some(v) = obj.borrow().fields.get(name) { return Ok(v.clone()); }
                }
                err!("Variable inconnue '{}'", name)
            }

            Expr::UnaryOp { op, expr } => {
                let v = self.eval(expr, env, this)?;
                match op {
                    UnaryOp::Neg => match v {
                        Value::Int(n)   => Ok(Value::Int(-n)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        _ => err!("- non applicable"),
                    },
                    UnaryOp::Not => match v {
                        Value::Bool(b) => Ok(Value::Bool(!b)),
                        _ => err!("! non applicable"),
                    },
                }
            }

            Expr::BinOp { left, op, right } => {
                let lv = self.eval(left, env, this.clone())?;
                let rv = self.eval(right, env, this)?;
                eval_binop(lv, op, rv)
            }

            Expr::FieldAccess { object, field } => {
                match self.eval(object, env, this)? {
                    Value::Object(rc) => rc.borrow().fields.get(field).cloned()
                        .ok_or_else(|| RuntimeError(format!("Champ inconnu '{}'", field))),
                    Value::Enum(ed)   => ed.fields.get(field).cloned()
                        .ok_or_else(|| RuntimeError(format!("Champ inconnu '{}'", field))),
                    _ => err!("Accès champ sur non-objet"),
                }
            }

            Expr::MethodCall { object, method, args } => {
                let obj = self.eval(object, env, this.clone())?;
                let args: Vec<Value> = args.iter()
                    .map(|a| self.eval(a, env, this.clone()))
                    .collect::<Result<_, _>>()?;
                match obj {
                    Value::Object(rc) => {
                        let cn = rc.borrow().class_name.clone();
                        let m = self.find_method(&cn, method)
                            .ok_or_else(|| RuntimeError(format!("Méthode inconnue '{}.{}()'", cn, method)))?;
                        if matches!(m.body.as_slice(), [Stmt::Builtin]) {
                            match (cn.as_str(), method.as_str()) {
                                (_, "equals") if args.len() == 1 => {
                                    let result = match &args[0] {
                                        Value::Object(other) => Rc::ptr_eq(&rc, other),
                                        _ => false,
                                    };
                                    Ok(Value::Bool(result))
                                }
                                ("ArrayList", "get") if args.len() == 1 => {
                                    let count = match rc.borrow().fields.get("count") {
                                        Some(Value::Int(n)) => *n as usize,
                                        _ => 0,
                                    };
                                    let data = match rc.borrow().fields.get("data").cloned() {
                                        Some(Value::Array(a)) => a,
                                        _ => return err!("ArrayList: champ 'data' introuvable"),
                                    };
                                    match &args[0] {
                                        Value::Int(i) => {
                                            if *i < 0 || *i as usize >= count {
                                                Ok(make_none())
                                            } else {
                                                Ok(make_some(data.borrow()[*i as usize].clone()))
                                            }
                                        }
                                        _ => err!("ArrayList.get() requiert un int"),
                                    }
                                }
                                ("ArrayList", "set") if args.len() == 2 => {
                                    let count = match rc.borrow().fields.get("count") {
                                        Some(Value::Int(n)) => *n as usize,
                                        _ => 0,
                                    };
                                    let data = match rc.borrow().fields.get("data").cloned() {
                                        Some(Value::Array(a)) => a,
                                        _ => return err!("ArrayList: champ 'data' introuvable"),
                                    };
                                    match &args[0] {
                                        Value::Int(i) => {
                                            let i = *i;
                                            if i < 0 || i as usize >= count {
                                                Ok(Value::Bool(false))
                                            } else {
                                                let producer = args[1].clone();
                                                let new_val = self.call_lambda(producer, vec![])?;
                                                data.borrow_mut()[i as usize] = new_val;
                                                Ok(Value::Bool(true))
                                            }
                                        }
                                        _ => err!("ArrayList.set() requiert un int comme index"),
                                    }
                                }
                                ("ArrayList", "contains") if args.len() == 1 => {
                                    let needle = &args[0];
                                    let count = match rc.borrow().fields.get("count") {
                                        Some(Value::Int(n)) => *n as usize,
                                        _ => 0,
                                    };
                                    let data = match rc.borrow().fields.get("data").cloned() {
                                        Some(Value::Array(a)) => a,
                                        _ => return err!("ArrayList: champ 'data' introuvable"),
                                    };
                                    let found = (0..count).any(|i| val_eq(&data.borrow()[i], needle));
                                    Ok(Value::Bool(found))
                                }
                                ("ArrayList", "indexOf") if args.len() == 1 => {
                                    let needle = &args[0];
                                    let count = match rc.borrow().fields.get("count") {
                                        Some(Value::Int(n)) => *n as usize,
                                        _ => 0,
                                    };
                                    let data = match rc.borrow().fields.get("data").cloned() {
                                        Some(Value::Array(a)) => a,
                                        _ => return err!("ArrayList: champ 'data' introuvable"),
                                    };
                                    let arr = data.borrow();
                                    if let Some(pos) = (0..count).find(|&i| val_eq(&arr[i], needle)) {
                                        Ok(make_some(Value::Int(pos as i64)))
                                    } else {
                                        Ok(make_none())
                                    }
                                }
                                ("ArrayList", "find") if args.len() == 1 => {
                                    let needle = &args[0];
                                    let count = match rc.borrow().fields.get("count") {
                                        Some(Value::Int(n)) => *n as usize,
                                        _ => 0,
                                    };
                                    let data = match rc.borrow().fields.get("data").cloned() {
                                        Some(Value::Array(a)) => a,
                                        _ => return err!("ArrayList: champ 'data' introuvable"),
                                    };
                                    let arr = data.borrow();
                                    if let Some(pos) = (0..count).find(|&i| val_eq(&arr[i], needle)) {
                                        Ok(make_some(arr[pos].clone()))
                                    } else {
                                        Ok(make_none())
                                    }
                                }
                                ("ArrayList", "toString") if args.is_empty() => {
                                    let count = match rc.borrow().fields.get("count") {
                                        Some(Value::Int(n)) => *n as usize,
                                        _ => 0,
                                    };
                                    let data = match rc.borrow().fields.get("data").cloned() {
                                        Some(Value::Array(a)) => a,
                                        _ => return err!("ArrayList: champ 'data' introuvable"),
                                    };
                                    let parts: Vec<String> = (0..count)
                                        .map(|i| data.borrow()[i].to_string())
                                        .collect();
                                    Ok(Value::Str(format!("ArrayList[{}]", parts.join(", "))))
                                }
                                ("HashSet", "toString") if args.is_empty() => {
                                    let map = match rc.borrow().fields.get("map").cloned() {
                                        Some(Value::HashMap(m)) => m,
                                        _ => return err!("HashSet: champ 'map' introuvable"),
                                    };
                                    let s = format!("HashSet{{{}}}", map.borrow().iter()
                                        .map(|(k, _)| k.to_string())
                                        .collect::<Vec<_>>().join(", "));
                                    Ok(Value::Str(s))
                                }
                                // ── RefArray<T> ───────────────────────────────
                                ("RefArray", "set") if args.len() == 1 => {
                                    let arr = match rc.borrow().fields.get("_array").cloned() {
                                        Some(Value::Array(a)) => a,
                                        _ => return err!("RefArray: champ '_array' invalide"),
                                    };
                                    let idx = match rc.borrow().fields.get("_index").cloned() {
                                        Some(Value::Int(i)) => i as usize,
                                        _ => return err!("RefArray: champ '_index' invalide"),
                                    };
                                    arr.borrow_mut()[idx] = args[0].clone();
                                    Ok(Value::Void)
                                }
                                ("RefArray", "get") if args.is_empty() => {
                                    let arr = match rc.borrow().fields.get("_array").cloned() {
                                        Some(Value::Array(a)) => a,
                                        _ => return err!("RefArray: champ '_array' invalide"),
                                    };
                                    let idx = match rc.borrow().fields.get("_index").cloned() {
                                        Some(Value::Int(i)) => i as usize,
                                        _ => return err!("RefArray: champ '_index' invalide"),
                                    };
                                    Ok(arr.borrow()[idx].clone())
                                }
                                // ── Flux standard (minilang.system) ──────────
                                ("StandardOutput", "write")     => io_write(&args, false, false),
                                ("StandardOutput", "writeLine") => io_write(&args, true,  false),
                                ("StandardOutput", "flush")     => io_flush(false),
                                ("StandardError",  "write")     => io_write(&args, false, true),
                                ("StandardError",  "writeLine") => io_write(&args, true,  true),
                                ("StandardError",  "flush")     => io_flush(true),
                                ("StandardInput",  "readLine")  => io_read_line(),
                                ("StandardInput",  "readChar")  => io_read_char(),
                                ("StandardInput",  "readAll")   => io_read_all(),
                                // ── Conversions string <-> byte[] (minilang.io) ──
                                ("Bytes", "encodeUtf8") if args.len() == 1 => {
                                    match &args[0] {
                                        Value::Str(s) => {
                                            let bytes: Vec<Value> = s.as_bytes().iter()
                                                .map(|b| Value::Byte(*b)).collect();
                                            Ok(Value::Array(Rc::new(RefCell::new(bytes))))
                                        }
                                        _ => err!("encodeUtf8() requiert une string"),
                                    }
                                }
                                ("Bytes", "decodeUtf8") if args.len() == 1 => {
                                    match &args[0] {
                                        Value::Array(a) => {
                                            let mut buf = Vec::with_capacity(a.borrow().len());
                                            for v in a.borrow().iter() {
                                                match v {
                                                    Value::Byte(b) => buf.push(*b),
                                                    _ => return err!("decodeUtf8() requiert un byte[]"),
                                                }
                                            }
                                            match String::from_utf8(buf) {
                                                Ok(s)  => Ok(make_ok(Value::Str(s))),
                                                Err(_) => Ok(io_err("Other",
                                                    Some("séquence UTF-8 invalide".to_string()))),
                                            }
                                        }
                                        _ => err!("decodeUtf8() requiert un byte[]"),
                                    }
                                }
                                _ => Err(RuntimeError(format!(
                                    "Méthode builtin inconnue '{}.{}()'", cn, method))),
                            }
                        } else {
                            self.call_method(&m, args, rc)
                        }
                    }
                    Value::Enum(ed) => {
                        // Builtins spéciaux sur les enums
                        if method == "hashCode" && args.is_empty() {
                            return Ok(Value::Int(val_hash(&Value::Enum(ed))));
                        }
                        let en = ed.enum_name.clone();
                        let m = self.find_method(&en, method)
                            .ok_or_else(|| RuntimeError(format!("Méthode inconnue '{}::{}()'", en, method)))?;
                        self.call_enum_method(&m, args, ed)
                    }
                    Value::Array(v) => {
                        match method.as_str() {
                            "get" => {
                                if args.len() != 1 { return err!("get() attend 1 argument"); }
                                match &args[0] {
                                    Value::Int(i) => {
                                        let data = v.borrow();
                                        if *i < 0 || *i as usize >= data.len() {
                                            Ok(make_none())
                                        } else {
                                            Ok(make_some(data[*i as usize].clone()))
                                        }
                                    }
                                    _ => err!("get() requiert un int"),
                                }
                            }
                            "set" => {
                                // set(index) -> Option<RefArray<T>>
                                if args.len() != 1 { return err!("set() attend 1 argument (l'index)"); }
                                match &args[0] {
                                    Value::Int(i) => {
                                        let i = *i;
                                        let data_len = v.borrow().len();
                                        if i < 0 || i as usize >= data_len {
                                            Ok(make_none())
                                        } else {
                                            let mut fields = HashMap::new();
                                            fields.insert("_array".to_string(), Value::Array(v.clone()));
                                            fields.insert("_index".to_string(), Value::Int(i));
                                            let ref_array = Value::Object(Rc::new(RefCell::new(ObjectData {
                                                class_name: "RefArray".to_string(),
                                                fields,
                                            })));
                                            Ok(make_some(ref_array))
                                        }
                                    }
                                    _ => err!("set() requiert un int"),
                                }
                            }
                            "length" => Ok(Value::Int(v.borrow().len() as i64)),
                            "contains" => {
                                if args.len() != 1 { return err!("contains() attend 1 argument"); }
                                let needle = &args[0];
                                let found = v.borrow().iter().any(|x| val_eq(x, needle));
                                Ok(Value::Bool(found))
                            }
                            "indexOf" => {
                                if args.len() != 1 { return err!("indexOf() attend 1 argument"); }
                                let needle = &args[0];
                                let data = v.borrow();
                                if let Some(pos) = data.iter().position(|x| val_eq(x, needle)) {
                                    Ok(make_some(Value::Int(pos as i64)))
                                } else {
                                    Ok(make_none())
                                }
                            }
                            "find" => {
                                if args.len() != 1 { return err!("find() attend 1 argument"); }
                                let needle = &args[0];
                                let data = v.borrow();
                                if let Some(found) = data.iter().find(|x| val_eq(x, needle)) {
                                    Ok(make_some(found.clone()))
                                } else {
                                    Ok(make_none())
                                }
                            }
                            "fill" => {
                                if args.len() != 1 { return err!("fill() attend 1 argument"); }
                                let val = args[0].clone();
                                let mut data = v.borrow_mut();
                                for x in data.iter_mut() { *x = val.clone(); }
                                Ok(Value::Void)
                            }
                            "forEach" => {
                                if args.len() != 1 { return err!("forEach() attend 1 argument"); }
                                let consumer = args[0].clone();
                                let items: Vec<Value> = v.borrow().clone();
                                for item in items {
                                    self.call_lambda(consumer.clone(), vec![item])?;
                                }
                                Ok(Value::Void)
                            }
                            _ => err!("Méthode inconnue '{}' sur Array", method),
                        }
                    }
                    Value::Str(ref s) => {
                        match method.as_str() {
                            "length" => Ok(Value::Int(s.chars().count() as i64)),
                            "isEmpty" => Ok(Value::Bool(s.is_empty())),
                            "charAt" => {
                                // charAt(index) -> Option<char>
                                if args.len() != 1 { return err!("charAt() attend 1 argument"); }
                                match &args[0] {
                                    Value::Int(i) => {
                                        match s.chars().nth(*i as usize) {
                                            Some(c) => Ok(make_some(Value::Char(c))),
                                            None    => Ok(make_none()),
                                        }
                                    }
                                    _ => err!("charAt() requiert un int"),
                                }
                            }
                            "contains" => {
                                if args.len() != 1 { return err!("contains() attend 1 argument"); }
                                match &args[0] {
                                    Value::Str(sub) => Ok(Value::Bool(s.contains(sub.as_str()))),
                                    _ => err!("contains() requiert une string"),
                                }
                            }
                            "substring" => {
                                if args.len() != 2 { return err!("substring() attend 2 arguments"); }
                                match (&args[0], &args[1]) {
                                    (Value::Int(start), Value::Int(end)) => {
                                        let start = *start as usize;
                                        let end   = *end   as usize;
                                        let chars: Vec<char> = s.chars().collect();
                                        if start > chars.len() || end > chars.len() || start > end {
                                            return err!("substring({}, {}): indices invalides (len={})", start, end, chars.len());
                                        }
                                        Ok(Value::Str(chars[start..end].iter().collect()))
                                    }
                                    _ => err!("substring() requiert deux int"),
                                }
                            }
                            "toUpperCase" => Ok(Value::Str(s.to_uppercase())),
                            "toLowerCase" => Ok(Value::Str(s.to_lowercase())),
                            "startsWith" => {
                                if args.len() != 1 { return err!("startsWith() attend 1 argument"); }
                                match &args[0] {
                                    Value::Str(prefix) => Ok(Value::Bool(s.starts_with(prefix.as_str()))),
                                    _ => err!("startsWith() requiert une string"),
                                }
                            }
                            "endsWith" => {
                                if args.len() != 1 { return err!("endsWith() attend 1 argument"); }
                                match &args[0] {
                                    Value::Str(suffix) => Ok(Value::Bool(s.ends_with(suffix.as_str()))),
                                    _ => err!("endsWith() requiert une string"),
                                }
                            }
                            "indexOf" => {
                                // indexOf(s) -> Option<int>
                                if args.len() != 1 { return err!("indexOf() attend 1 argument"); }
                                match &args[0] {
                                    Value::Str(needle) => {
                                        match s.find(needle.as_str()) {
                                            Some(b) => Ok(make_some(Value::Int(s[..b].chars().count() as i64))),
                                            None    => Ok(make_none()),
                                        }
                                    }
                                    _ => err!("indexOf() requiert une string"),
                                }
                            }
                            "trim" => Ok(Value::Str(s.trim().to_string())),
                            "replace" => {
                                if args.len() != 2 { return err!("replace() attend 2 arguments"); }
                                match (&args[0], &args[1]) {
                                    (Value::Str(old), Value::Str(new)) => {
                                        Ok(Value::Str(s.replace(old.as_str(), new.as_str())))
                                    }
                                    _ => err!("replace() requiert deux string"),
                                }
                            }
                            "equals" => {
                                if args.len() != 1 { return err!("equals() attend 1 argument"); }
                                match &args[0] {
                                    Value::Str(other) => Ok(Value::Bool(s == other)),
                                    _ => Ok(Value::Bool(false)),
                                }
                            }
                            "split" => {
                                // split(sep) -> List<string>  (retourne un ArrayList<string>)
                                if args.len() != 1 { return err!("split() attend 1 argument"); }
                                match &args[0] {
                                    Value::Str(sep) => {
                                        let parts: Vec<Value> = if sep.is_empty() {
                                            s.chars().map(|c| Value::Str(c.to_string())).collect()
                                        } else {
                                            s.split(sep.as_str()).map(|p| Value::Str(p.to_string())).collect()
                                        };
                                        let count = parts.len() as i64;
                                        let mut fields = HashMap::new();
                                        fields.insert("data".to_string(),
                                            Value::Array(Rc::new(RefCell::new(parts))));
                                        fields.insert("count".to_string(), Value::Int(count));
                                        Ok(Value::Object(Rc::new(RefCell::new(ObjectData {
                                            class_name: "ArrayList".to_string(),
                                            fields,
                                        }))))
                                    }
                                    _ => err!("split() requiert une string"),
                                }
                            }
                            "hashCode" => Ok(Value::Int(val_hash(&Value::Str(s.clone())))),
                            _ => {
                                // Fallthrough vers la définition minilang de la classe String
                                match self.find_method("String", method) {
                                    Some(m) if !matches!(m.body.as_slice(), [Stmt::Builtin]) =>
                                        self.call_primitive_method(&m, args, Value::Str(s.clone())),
                                    _ => err!("Méthode inconnue '{}' sur string", method),
                                }
                            }
                        }
                    }
                    Value::Char(c) => {
                        match method.as_str() {
                            "isLetter"    => Ok(Value::Bool(c.is_alphabetic())),
                            "isDigit"     => Ok(Value::Bool(c.is_ascii_digit())),
                            "isWhitespace"=> Ok(Value::Bool(c.is_whitespace())),
                            "isUpperCase" => Ok(Value::Bool(c.is_uppercase())),
                            "isLowerCase" => Ok(Value::Bool(c.is_lowercase())),
                            "toUpperCase" => Ok(Value::Char(c.to_uppercase().next().unwrap_or(c))),
                            "toLowerCase" => Ok(Value::Char(c.to_lowercase().next().unwrap_or(c))),
                            "toInt"       => Ok(Value::Int(c as i64)),
                            "toString"    => Ok(Value::Str(c.to_string())),
                            "equals" => {
                                if args.len() != 1 { return err!("equals() attend 1 argument"); }
                                match &args[0] {
                                    Value::Char(o) => Ok(Value::Bool(c == *o)),
                                    _ => Ok(Value::Bool(false)),
                                }
                            }
                            "hashCode" => Ok(Value::Int(c as i64)),
                            _ => err!("Méthode inconnue '{}' sur char", method),
                        }
                    }
                    Value::Bool(b) => {
                        match method.as_str() {
                            "toString" => Ok(Value::Str(b.to_string())),
                            "equals"   => {
                                if args.len() != 1 { return err!("equals() attend 1 argument"); }
                                match &args[0] {
                                    Value::Bool(o) => Ok(Value::Bool(b == *o)),
                                    _ => Ok(Value::Bool(false)),
                                }
                            }
                            "and" => {
                                if args.len() != 1 { return err!("and() attend 1 argument"); }
                                match &args[0] {
                                    Value::Bool(o) => Ok(Value::Bool(b && *o)),
                                    _ => err!("and() requiert un bool"),
                                }
                            }
                            "or" => {
                                if args.len() != 1 { return err!("or() attend 1 argument"); }
                                match &args[0] {
                                    Value::Bool(o) => Ok(Value::Bool(b || *o)),
                                    _ => err!("or() requiert un bool"),
                                }
                            }
                            "not" => Ok(Value::Bool(!b)),
                            "hashCode" => Ok(Value::Int(if b { 1 } else { 0 })),
                            _ => err!("Méthode inconnue '{}' sur bool", method),
                        }
                    }
                    Value::Int(n) => {
                        match method.as_str() {
                            "toString"       => Ok(Value::Str(n.to_string())),
                            "toBinaryString" => Ok(Value::Str(format!("{:b}", n))),
                            "abs"            => Ok(Value::Int(n.abs())),
                            "isPositive"     => Ok(Value::Bool(n > 0)),
                            "isNegative"     => Ok(Value::Bool(n < 0)),
                            "isZero"         => Ok(Value::Bool(n == 0)),
                            "isEven"         => Ok(Value::Bool(n % 2 == 0)),
                            "isOdd"          => Ok(Value::Bool(n % 2 != 0)),
                            "toFloat"        => Ok(Value::Float(n as f64)),
                            "toDouble"       => Ok(Value::Float(n as f64)),
                            "toByte"         => {
                                if n >= 0 && n <= 255 { Ok(make_some(Value::Byte(n as u8))) }
                                else { Ok(make_none()) }
                            }
                            "min" => {
                                if args.len() != 1 { return err!("min() attend 1 argument"); }
                                match &args[0] {
                                    Value::Int(o) => Ok(Value::Int(n.min(*o))),
                                    _ => err!("min() requiert un int"),
                                }
                            }
                            "max" => {
                                if args.len() != 1 { return err!("max() attend 1 argument"); }
                                match &args[0] {
                                    Value::Int(o) => Ok(Value::Int(n.max(*o))),
                                    _ => err!("max() requiert un int"),
                                }
                            }
                            "pow" => {
                                if args.len() != 1 { return err!("pow() attend 1 argument"); }
                                match &args[0] {
                                    Value::Int(e) if *e >= 0 => Ok(Value::Int(i64::pow(n, *e as u32))),
                                    Value::Int(e) => err!("pow() : exposant négatif {}", e),
                                    _ => err!("pow() requiert un int"),
                                }
                            }
                            "compareTo" => {
                                if args.len() != 1 { return err!("compareTo() attend 1 argument"); }
                                match &args[0] {
                                    Value::Int(o) => Ok(Value::Int(n.cmp(o) as i64)),
                                    _ => err!("compareTo() requiert un int"),
                                }
                            }
                            "equals" => {
                                if args.len() != 1 { return err!("equals() attend 1 argument"); }
                                match &args[0] {
                                    Value::Int(o) => Ok(Value::Bool(n == *o)),
                                    _ => Ok(Value::Bool(false)),
                                }
                            }
                            "hashCode" => Ok(Value::Int(n)),
                            _ => err!("Méthode inconnue '{}' sur int", method),
                        }
                    }
                    Value::Byte(b) => {
                        match method.as_str() {
                            "toInt"    => Ok(Value::Int(b as i64)),
                            "toString" => Ok(Value::Str(b.to_string())),
                            "equals" => {
                                if args.len() != 1 { return err!("equals() attend 1 argument"); }
                                match &args[0] {
                                    Value::Byte(o) => Ok(Value::Bool(b == *o)),
                                    _ => Ok(Value::Bool(false)),
                                }
                            }
                            "hashCode" => Ok(Value::Int(b as i64)),
                            _ => err!("Méthode inconnue '{}' sur byte", method),
                        }
                    }
                    Value::Float(f) => {
                        match method.as_str() {
                            "toString"   => Ok(Value::Str(f.to_string())),
                            "abs"        => Ok(Value::Float(f.abs())),
                            "floor"      => Ok(Value::Float(f.floor())),
                            "ceil"       => Ok(Value::Float(f.ceil())),
                            "round"      => Ok(Value::Float(f.round())),
                            "isPositive" => Ok(Value::Bool(f > 0.0)),
                            "isNegative" => Ok(Value::Bool(f < 0.0)),
                            "isNaN"      => Ok(Value::Bool(f.is_nan())),
                            "toInt"      => Ok(Value::Int(f as i64)),
                            "toFloat"    => Ok(Value::Float(f)),
                            "toDouble"   => Ok(Value::Float(f)),
                            "min" => {
                                if args.len() != 1 { return err!("min() attend 1 argument"); }
                                match &args[0] {
                                    Value::Float(o) => Ok(Value::Float(f.min(*o))),
                                    _ => err!("min() requiert un float/double"),
                                }
                            }
                            "max" => {
                                if args.len() != 1 { return err!("max() attend 1 argument"); }
                                match &args[0] {
                                    Value::Float(o) => Ok(Value::Float(f.max(*o))),
                                    _ => err!("max() requiert un float/double"),
                                }
                            }
                            "equals" => {
                                if args.len() != 1 { return err!("equals() attend 1 argument"); }
                                match &args[0] {
                                    Value::Float(o) => Ok(Value::Bool((f - o).abs() < 1e-12)),
                                    _ => Ok(Value::Bool(false)),
                                }
                            }
                            "hashCode" => Ok(Value::Int(f.to_bits() as i64)),
                            _ => err!("Méthode inconnue '{}' sur float/double", method),
                        }
                    }
                    Value::HashMap(v) => {
                        match method.as_str() {
                            "put" => {
                                if args.len() != 2 { return err!("put() attend 2 arguments"); }
                                let key = args[0].clone();
                                let val = args[1].clone();
                                let mut data = v.borrow_mut();
                                if let Some(entry) = data.iter_mut().find(|(k, _)| val_eq(k, &key)) {
                                    entry.1 = val;
                                } else {
                                    data.push((key, val));
                                }
                                Ok(Value::Void)
                            }
                            "get" => {
                                if args.len() != 1 { return err!("get() attend 1 argument"); }
                                let key = &args[0];
                                let data = v.borrow();
                                if let Some((_, val)) = data.iter().find(|(k, _)| val_eq(k, key)) {
                                    Ok(make_some(val.clone()))
                                } else {
                                    Ok(make_none())
                                }
                            }
                            "containsKey" => {
                                if args.len() != 1 { return err!("containsKey() attend 1 argument"); }
                                let found = v.borrow().iter().any(|(k, _)| val_eq(k, &args[0]));
                                Ok(Value::Bool(found))
                            }
                            "size"    => Ok(Value::Int(v.borrow().len() as i64)),
                            "isEmpty" => Ok(Value::Bool(v.borrow().is_empty())),
                            "remove" => {
                                if args.len() != 1 { return err!("remove() attend 1 argument"); }
                                let key = args[0].clone();
                                let mut data = v.borrow_mut();
                                if let Some(pos) = data.iter().position(|(k, _)| val_eq(k, &key)) {
                                    data.remove(pos);
                                    Ok(Value::Bool(true))
                                } else {
                                    Ok(Value::Bool(false))
                                }
                            }
                            "clear" => { v.borrow_mut().clear(); Ok(Value::Void) }
                            "keys" => {
                                if !args.is_empty() { return err!("keys() ne prend pas d'arguments"); }
                                let pairs = v.borrow();
                                let keys: Vec<Value> = pairs.iter().map(|(k, _)| k.clone()).collect();
                                let count = keys.len() as i64;
                                let arr = Value::Array(Rc::new(RefCell::new(keys)));
                                let mut fields = HashMap::new();
                                fields.insert("data".to_string(), arr);
                                fields.insert("count".to_string(), Value::Int(count));
                                Ok(Value::Object(Rc::new(RefCell::new(ObjectData {
                                    class_name: "ArrayList".to_string(),
                                    fields,
                                }))))
                            }
                            "entries" => {
                                if !args.is_empty() { return err!("entries() ne prend pas d'arguments"); }
                                let pairs = v.borrow();
                                let entries: Vec<Value> = pairs.iter().map(|(k, val)| {
                                    let mut fields = HashMap::new();
                                    fields.insert("first".to_string(),  k.clone());
                                    fields.insert("second".to_string(), val.clone());
                                    Value::Object(Rc::new(RefCell::new(ObjectData {
                                        class_name: "Pair".to_string(),
                                        fields,
                                    })))
                                }).collect();
                                let count = entries.len() as i64;
                                let arr = Value::Array(Rc::new(RefCell::new(entries)));
                                let mut fields = HashMap::new();
                                fields.insert("data".to_string(),  arr);
                                fields.insert("count".to_string(), Value::Int(count));
                                Ok(Value::Object(Rc::new(RefCell::new(ObjectData {
                                    class_name: "ArrayList".to_string(),
                                    fields,
                                }))))
                            }
                            "forEach" => {
                                if args.len() != 1 { return err!("forEach() attend 1 argument"); }
                                let consumer = args[0].clone();
                                let pairs: Vec<(Value, Value)> = v.borrow().clone();
                                for (k, val) in pairs {
                                    self.call_lambda(consumer.clone(), vec![k, val])?;
                                }
                                Ok(Value::Void)
                            }
                            "toString" => {
                                let s = format!("HashMap{{{}}}", v.borrow().iter()
                                    .map(|(k, val)| format!("{}={}", k, val))
                                    .collect::<Vec<_>>().join(", "));
                                Ok(Value::Str(s))
                            }
                            _ => err!("Méthode inconnue '{}' sur HashMap", method),
                        }
                    }
                    _ => err!("Appel de méthode sur non-objet"),
                }
            }

            Expr::FunctionCall { name, args } => {
                let args: Vec<Value> = args.iter()
                    .map(|a| self.eval(a, env, this.clone()))
                    .collect::<Result<_, _>>()?;
                // Lambda dans une variable locale ?
                if let Some(lam) = env.get(name) {
                    if let Value::Lambda { .. } = &lam {
                        return self.call_lambda(lam, args);
                    }
                }
                // Méthode de la classe/enum courante
                if let Some(rc) = this {
                    let cn = rc.borrow().class_name.clone();
                    if let Some(m) = self.find_method(&cn, name) {
                        return self.call_method(&m, args, rc);
                    }
                }
                if name == "panic" {
                    let msg = args.into_iter().next()
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "panic".to_string());
                    return err!("{}", msg);
                }
                // ── Assertions builtin (système de tests) ────────────────────
                match name.as_str() {
                    "assertTrue" => {
                        return match args.first() {
                            Some(Value::Bool(true))  => Ok(Value::Void),
                            Some(Value::Bool(false)) => err!("assertTrue : la condition est fausse"),
                            _ => err!("assertTrue : condition non-bool"),
                        };
                    }
                    "assertFalse" => {
                        return match args.first() {
                            Some(Value::Bool(false)) => Ok(Value::Void),
                            Some(Value::Bool(true))  => err!("assertFalse : la condition est vraie"),
                            _ => err!("assertFalse : condition non-bool"),
                        };
                    }
                    "assertEquals" => {
                        if args.len() != 2 { return err!("assertEquals attend 2 arguments"); }
                        return if val_eq(&args[0], &args[1]) { Ok(Value::Void) }
                               else { err!("assertEquals : {} ≠ {}", args[0], args[1]) };
                    }
                    "assertNotEquals" => {
                        if args.len() != 2 { return err!("assertNotEquals attend 2 arguments"); }
                        return if !val_eq(&args[0], &args[1]) { Ok(Value::Void) }
                               else { err!("assertNotEquals : les deux valeurs valent {}", args[0]) };
                    }
                    "fail" => {
                        let msg = args.first().map(|v| v.to_string())
                            .unwrap_or_else(|| "fail".to_string());
                        return err!("fail : {}", msg);
                    }
                    _ => {}
                }
                // Fonction de haut niveau
                if let Some(func) = self.funcs.get(name.as_str()).cloned() {
                    let mut fenv = Env::new();
                    for (p, v) in func.params.iter().zip(args.into_iter()) {
                        fenv.declare(p.name.clone(), v);
                    }
                    return match self.exec_body(&func.body, &mut fenv, None)? {
                        Flow::Return(v) => Ok(v),
                        _               => Ok(Value::Void),
                    };
                }
                err!("Fonction inconnue '{}'", name)
            }

            Expr::New { class_name, args, .. } => {
                match class_name.as_str() {
                    "HashMap"  => return Ok(Value::HashMap(Rc::new(RefCell::new(vec![])))),
                    _ => {}
                }
                let obj = self.instantiate(class_name)?;
                let rc = match &obj { Value::Object(r) => r.clone(), _ => unreachable!() };
                let ctors = self.classes.get(class_name)
                    .map(|c| c.constructors.clone()).unwrap_or_default();
                if !ctors.is_empty() {
                    let ctor = ctors.iter().find(|c| c.params.len() == args.len()).cloned()
                        .ok_or_else(|| RuntimeError(format!(
                            "Pas de constructeur à {} arg(s) pour '{}'", args.len(), class_name)))?;
                    let eargs: Vec<Value> = args.iter()
                        .map(|a| self.eval(a, env, this.clone())).collect::<Result<_, _>>()?;
                    let mut ce = Env::new(); ce.push();
                    for (p, v) in ctor.params.iter().zip(eargs) { ce.declare(p.name.clone(), v); }
                    self.exec_body(&ctor.body, &mut ce, Some(rc))?;
                }
                Ok(obj)
            }

            // ── inject T — résolution d'un service (singleton) ───────────────
            // Le typechecker garantit : binding unique, pas de cycle.
            Expr::Inject(ty) => {
                let name = match ty {
                    Type::UserDefined(n) => n.clone(),
                    other => return err!("inject : type non injectable {}", other),
                };
                let concrete = self.resolve_service_class(&name)?;
                self.get_or_create_service(&concrete)
            }

            Expr::EnumConstructor { enum_name, variant, args, .. } => {
                let ed = self.enums.get(enum_name)
                    .ok_or_else(|| RuntimeError(format!("Enum inconnu '{}'", enum_name)))?.clone();
                let vd = ed.variants.iter().find(|v| &v.name == variant)
                    .ok_or_else(|| RuntimeError(format!("Variante '{}' inconnue", variant)))?.clone();
                let eargs: Vec<Value> = args.iter()
                    .map(|a| self.eval(a, env, this.clone())).collect::<Result<_, _>>()?;
                let mut fields = HashMap::new();
                let mut field_order = Vec::new();
                for (p, v) in vd.fields.iter().zip(eargs) {
                    fields.insert(p.name.clone(), v);
                    field_order.push(p.name.clone());
                }
                Ok(Value::Enum(Rc::new(EnumData {
                    enum_name: enum_name.clone(), variant_name: variant.clone(),
                    fields, field_order,
                })))
            }

            // ── Navigation sûre : ?.field et ?.method() ──────────────────────

            Expr::SafeFieldAccess { object, field } => {
                let v = self.eval(object, env, this)?;
                match v {
                    Value::Enum(ref ed) if ed.enum_name == "Option" => {
                        if ed.variant_name == "None" { return Ok(make_none()); }
                        let inner = ed.fields.get("value")
                            .ok_or_else(|| RuntimeError("Option::Some sans champ 'value'".into()))?.clone();
                        match inner {
                            Value::Object(rc) => {
                                let fv = rc.borrow().fields.get(field).cloned()
                                    .ok_or_else(|| RuntimeError(format!("Champ inconnu '{}'", field)))?;
                                Ok(make_some(fv))
                            }
                            _ => err!("?. : la valeur dans Some n'est pas un objet"),
                        }
                    }
                    _ => err!("?. requiert une valeur Option"),
                }
            }

            Expr::SafeMethodCall { object, method, args } => {
                let v = self.eval(object, env, this.clone())?;
                let eargs: Vec<Value> = args.iter()
                    .map(|a| self.eval(a, env, this.clone()))
                    .collect::<Result<_, _>>()?;
                match v {
                    Value::Enum(ref ed) if ed.enum_name == "Option" => {
                        if ed.variant_name == "None" { return Ok(make_none()); }
                        let inner = ed.fields.get("value")
                            .ok_or_else(|| RuntimeError("Option::Some sans champ 'value'".into()))?.clone();
                        let result = match inner {
                            Value::Object(rc) => {
                                let cn = rc.borrow().class_name.clone();
                                let m = self.find_method(&cn, method)
                                    .ok_or_else(|| RuntimeError(format!("Méthode inconnue '{}'", method)))?;
                                self.call_method(&m, eargs, rc)?
                            }
                            Value::Enum(inner_ed) => {
                                let en = inner_ed.enum_name.clone();
                                let m = self.find_method(&en, method)
                                    .ok_or_else(|| RuntimeError(format!("Méthode inconnue '{}'", method)))?;
                                self.call_enum_method(&m, eargs, inner_ed)?
                            }
                            _ => return err!("?. : la valeur dans Some n'est pas un objet"),
                        };
                        Ok(make_some(result))
                    }
                    _ => err!("?. requiert une valeur Option"),
                }
            }

            // ── Null coalescing : expr ?? default ─────────────────────────────

            Expr::NullCoalesce { expr, default } => {
                let v = self.eval(expr, env, this.clone())?;
                match &v {
                    Value::Enum(ed) if ed.enum_name == "Option"
                        && ed.variant_name == "None" =>
                    {
                        self.eval(default, env, this)
                    }
                    Value::Enum(ed) if ed.enum_name == "Option"
                        && ed.variant_name == "Some" =>
                    {
                        Ok(ed.fields.get("value").cloned().unwrap_or(Value::Null))
                    }
                    _ => Ok(v),
                }
            }

            // ── Lambda — crée une fermeture avec capture lexicale ─────────────
            // Capture les variables de l'env ET les champs de `this`
            // (nécessaire pour les lambdas créées à l'intérieur d'une méthode
            // qui accèdent aux champs de l'objet sans préfixe `this.`).
            Expr::Lambda { params, body } => {
                let mut captured = env.snapshot();
                if let Some(obj) = &this {
                    for (k, v) in &obj.borrow().fields {
                        // Les champs de this complètent la capture (sans écraser
                        // les variables locales qui auraient le même nom)
                        captured.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
                debug!("lambda capturée ({} vars)", captured.len());
                Ok(Value::Lambda {
                    params:   params.clone(),
                    body:     body.clone(),
                    captured,
                })
            }

            // ── Appel d'une expression lambda : f(1, 2)  ou  ((x)=>x+1)(5) ───
            Expr::LambdaCall { callee, args } => {
                let lam = self.eval(callee, env, this.clone())?;
                let eargs: Vec<Value> = args.iter()
                    .map(|a| self.eval(a, env, this.clone()))
                    .collect::<Result<_, _>>()?;
                self.call_lambda(lam, eargs)
            }

            // ── Tableau littéral : new T[]{a, b, ...} ────────────────────────
            Expr::ArrayLit { elem_type: _, elements } => {
                let mut vals = Vec::new();
                for e in elements {
                    vals.push(self.eval(e, env, this.clone())?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(vals))))
            }

            // ── Nouveau tableau de taille n : new T[n] ou new T[n](fill) ──────
            Expr::ArrayNew { elem_type, size, fill } => {
                let n = match self.eval(size, env, this.clone())? {
                    Value::Int(n) if n >= 0 => n as usize,
                    Value::Int(n) => return err!("Taille de tableau négative : {}", n),
                    _ => return err!("Taille de tableau doit être un entier"),
                };
                let init = match fill {
                    Some(f) => self.eval(f, env, this)?,
                    None    => Self::default_value(elem_type),
                };
                Ok(Value::Array(Rc::new(RefCell::new(vec![init; n]))))
            }

            // ── Accès indexé : arr[i] — retourne Option<T> ───────────────────
            Expr::Index { object, index } => {
                let arr = self.eval(object, env, this.clone())?;
                let idx = self.eval(index, env, this)?;
                match (arr, idx) {
                    (Value::Array(v), Value::Int(i)) => {
                        let data = v.borrow();
                        if i < 0 || i as usize >= data.len() {
                            Ok(make_none())
                        } else {
                            Ok(make_some(data[i as usize].clone()))
                        }
                    }
                    _ => err!("Accès index sur non-tableau"),
                }
            }
        }
    }

    // ── Injection de dépendances ──────────────────────────────────────────────

    /// true si le paramètre est un slot de dépendance (interface ou classe
    /// service) — même classification que dans le typechecker. Tout autre type
    /// est un slot de configuration, rempli par les valeurs du `with`.
    fn is_dep_slot(&self, ty: &Type) -> bool {
        match ty {
            Type::UserDefined(n) =>
                self.interfaces.contains(n)
                || self.classes.get(n).map(|c| c.is_service).unwrap_or(false),
            _ => false,
        }
    }

    /// Résout un nom injectable vers la classe service concrète :
    /// la classe elle-même si elle est `service`, sinon le binding explicite
    /// d'un module, sinon l'unique service implémentant l'interface
    /// (l'unicité est garantie par le typechecker).
    fn resolve_service_class(&self, name: &str) -> Result<String, RuntimeError> {
        if let Some(c) = self.classes.get(name) {
            if c.is_service { return Ok(name.to_string()); }
        }
        if let Some(s) = self.binds_to.get(name) { return Ok(s.clone()); }
        let mut impls: Vec<&String> = self.classes.values()
            .filter(|c| c.is_service && self.class_conforms(&c.name, name))
            .map(|c| &c.name)
            .collect();
        impls.sort();
        match impls.first() {
            Some(n) => Ok((*n).clone()),
            None    => err!("Aucun service pour '{}'", name),
        }
    }

    /// true si la classe `cn` est conforme à l'interface `iface` : elle
    /// l'implémente directement, ou implémente une interface qui en hérite
    /// (transitif), ou hérite d'une classe conforme.
    fn class_conforms(&self, cn: &str, iface: &str) -> bool {
        let Some(c) = self.classes.get(cn) else { return false };
        for i in &c.implements {
            if i == iface || self.iface_extends(i, iface) { return true; }
        }
        if let Some(p) = &c.parent {
            if self.class_conforms(p, iface) { return true; }
        }
        false
    }

    /// true si l'interface `sub` étend (transitivement) `sup`.
    fn iface_extends(&self, sub: &str, sup: &str) -> bool {
        if sub == sup { return true; }
        if let Some(parents) = self.iface_parents.get(sub) {
            for p in parents {
                if self.iface_extends(p, sup) { return true; }
            }
        }
        false
    }

    /// Retourne l'instance du service `cn`, en l'instanciant (ainsi que ses
    /// dépendances, récursivement) si nécessaire. Les services singletons sont
    /// mémorisés ; les services `transient` sont recréés à chaque injection.
    /// Les services sont des `Rc<RefCell<…>>` : le clone partage l'instance.
    fn get_or_create_service(&mut self, cn: &str) -> Result<Value, RuntimeError> {
        let is_transient = self.classes.get(cn).map(|c| c.is_transient).unwrap_or(false);
        if !is_transient {
            if let Some(v) = self.singletons.get(cn) { return Ok(v.clone()); }
        }
        debug!("inject : création du service '{}'", cn);
        let ctor = self.classes.get(cn).and_then(|c| c.constructors.first().cloned());
        let with = self.with_values.get(cn).cloned().unwrap_or_default();
        let mut with_iter = with.into_iter();
        // Résoudre les arguments du constructeur : dépendances injectées
        // (ordre topologique implicite) + valeurs de configuration du `with`
        let mut arg_vals: Vec<Value> = vec![];
        if let Some(ctor) = &ctor {
            for p in &ctor.params {
                if self.is_dep_slot(&p.ty) {
                    let dep = match &p.ty {
                        Type::UserDefined(n) => n.clone(),
                        _ => unreachable!("is_dep_slot ne matche que UserDefined"),
                    };
                    let concrete = self.resolve_service_class(&dep)?;
                    arg_vals.push(self.get_or_create_service(&concrete)?);
                } else {
                    let expr = with_iter.next().ok_or_else(|| RuntimeError(format!(
                        "Service '{}' : valeur de configuration manquante pour '{}'",
                        cn, p.name)))?;
                    let mut wenv = Env::new();
                    arg_vals.push(self.eval(&expr, &mut wenv, None)?);
                }
            }
        }
        let obj = self.instantiate(cn)?;
        let rc = match &obj { Value::Object(r) => r.clone(), _ => unreachable!() };
        if let Some(ctor) = ctor {
            let mut ce = Env::new(); ce.push();
            for (p, v) in ctor.params.iter().zip(arg_vals) { ce.declare(p.name.clone(), v); }
            self.exec_body(&ctor.body, &mut ce, Some(rc))?;
        }
        if !is_transient {
            self.singletons.insert(cn.to_string(), obj.clone());
        }
        Ok(obj)
    }

    // ── Appel d'une valeur lambda ─────────────────────────────────────────────

    fn call_lambda(&mut self, lam: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        match lam {
            Value::Lambda { params, body, captured } => {
                if args.len() != params.len() {
                    return err!("Lambda : {} arg(s) attendus, {} fournis", params.len(), args.len());
                }
                // Environnement = variables capturées + paramètres
                let mut env = Env::new();
                env.push();
                for (k, v) in &captured { env.declare(k.clone(), v.clone()); }
                for (p, v) in params.iter().zip(args) { env.set(p.clone(), v); }

                match body {
                    LambdaBody::Expr(e) => {
                        // Corps expression : évaluation directe, retour implicite
                        self.eval(&e, &mut env, None)
                    }
                    LambdaBody::Block(stmts) => {
                        match self.exec_body(&stmts, &mut env, None)? {
                            Flow::Return(v) => Ok(v),
                            _               => Ok(Value::Void),
                        }
                    }
                }
            }
            other => err!("Appel sur non-lambda : {}", other),
        }
    }

    // ── Appel de méthode de classe ────────────────────────────────────────────

    fn call_method(
        &mut self, m: &Method, args: Vec<Value>, this: Rc<RefCell<ObjectData>>,
    ) -> Result<Value, RuntimeError> {
        if args.len() != m.params.len() {
            return err!("{}() : {} arg(s) attendus", m.name, m.params.len());
        }
        debug!("→ {}", m.name);
        let mut env = Env::new(); env.push();
        for (p, v) in m.params.iter().zip(args) { env.declare(p.name.clone(), v); }
        match self.exec_body(&m.body.clone(), &mut env, Some(this))? {
            Flow::Return(v) => Ok(v),
            _               => Ok(Value::Void),
        }
    }

    // ── Appel de méthode d'enum ───────────────────────────────────────────────
    // `this` = Value::Enum stocké dans l'env sous la clé "this"
    // (résout le bug où match this { Variant => } ne matchait jamais)

    fn call_enum_method(
        &mut self, m: &Method, args: Vec<Value>, ed: Rc<EnumData>,
    ) -> Result<Value, RuntimeError> {
        if args.len() != m.params.len() {
            return err!("{}() : {} arg(s) attendus", m.name, m.params.len());
        }
        debug!("→ enum::{}", m.name);
        let mut env = Env::new(); env.push();
        env.declare("this".to_string(), Value::Enum(ed));
        for (p, v) in m.params.iter().zip(args) { env.declare(p.name.clone(), v); }
        match self.exec_body(&m.body.clone(), &mut env, None)? {
            Flow::Return(v) => Ok(v),
            _               => Ok(Value::Void),
        }
    }

    // ── Appel de méthode minilang sur un type primitif ────────────────────────
    // `this` = valeur primitive (Value::Str, Value::Int, …) stockée dans l'env.
    // Permet d'écrire des méthodes String / Integer / … en minilang pur.

    fn call_primitive_method(
        &mut self, m: &Method, args: Vec<Value>, this_val: Value,
    ) -> Result<Value, RuntimeError> {
        if args.len() != m.params.len() {
            return err!("{}() : {} arg(s) attendus", m.name, m.params.len());
        }
        debug!("→ primitive::{}", m.name);
        let mut env = Env::new(); env.push();
        env.declare("this".to_string(), this_val);
        for (p, v) in m.params.iter().zip(args) { env.declare(p.name.clone(), v); }
        match self.exec_body(&m.body.clone(), &mut env, None)? {
            Flow::Return(v) => Ok(v),
            _               => Ok(Value::Void),
        }
    }
}

// ── Helpers Option ────────────────────────────────────────────────────────────

fn make_none() -> Value {
    Value::Enum(Rc::new(EnumData {
        enum_name: "Option".to_string(), variant_name: "None".to_string(),
        fields: HashMap::new(), field_order: vec![],
    }))
}

fn make_some(v: Value) -> Value {
    let mut fields = HashMap::new();
    fields.insert("value".to_string(), v);
    Value::Enum(Rc::new(EnumData {
        enum_name: "Option".to_string(), variant_name: "Some".to_string(),
        fields, field_order: vec!["value".to_string()],
    }))
}

// ── Helpers Result / I/O ────────────────────────────────────────────────────

/// `Result::Ok(value)`
fn make_ok(value: Value) -> Value {
    let mut fields = HashMap::new();
    fields.insert("value".to_string(), value);
    Value::Enum(Rc::new(EnumData {
        enum_name: "Result".to_string(), variant_name: "Ok".to_string(),
        fields, field_order: vec!["value".to_string()],
    }))
}

/// `Result<Unit, IoError>::Ok(new Unit())` — succès d'I/O sans valeur.
fn ok_unit() -> Value {
    let unit = Value::Object(Rc::new(RefCell::new(ObjectData {
        class_name: "Unit".to_string(), fields: HashMap::new(),
    })));
    make_ok(unit)
}

/// `Result::Err(IoError::<variant>(message?))`
fn io_err(variant: &str, message: Option<String>) -> Value {
    let mut ife = HashMap::new();
    let mut order = vec![];
    if let Some(m) = message {
        ife.insert("message".to_string(), Value::Str(m));
        order.push("message".to_string());
    }
    let io = Value::Enum(Rc::new(EnumData {
        enum_name: "IoError".to_string(), variant_name: variant.to_string(),
        fields: ife, field_order: order,
    }));
    let mut fields = HashMap::new();
    fields.insert("error".to_string(), io);
    Value::Enum(Rc::new(EnumData {
        enum_name: "Result".to_string(), variant_name: "Err".to_string(),
        fields, field_order: vec!["error".to_string()],
    }))
}

/// Écrit `args[0]` (une string) sur stdout/stderr, avec ou sans saut de ligne.
/// Renvoie un `Result<Unit, IoError>` minilang (Ok, ou Err en cas d'échec réel).
fn io_write(args: &[Value], newline: bool, to_stderr: bool) -> Result<Value, RuntimeError> {
    use std::io::Write;
    let s = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return err!("write() requiert une string"),
    };
    let res = if to_stderr {
        let mut h = std::io::stderr();
        (if newline { writeln!(h, "{}", s) } else { write!(h, "{}", s) }).and_then(|_| h.flush())
    } else {
        let mut h = std::io::stdout();
        (if newline { writeln!(h, "{}", s) } else { write!(h, "{}", s) }).and_then(|_| h.flush())
    };
    match res {
        Ok(())  => Ok(ok_unit()),
        Err(e)  => Ok(io_err("WriteFailed", Some(e.to_string()))),
    }
}

fn io_flush(to_stderr: bool) -> Result<Value, RuntimeError> {
    use std::io::Write;
    let res = if to_stderr { std::io::stderr().flush() } else { std::io::stdout().flush() };
    match res {
        Ok(())  => Ok(ok_unit()),
        Err(e)  => Ok(io_err("WriteFailed", Some(e.to_string()))),
    }
}

/// Lit une ligne sur stdin (sans le saut de ligne final).
/// `Result<Option<string>, IoError>` : Ok(Some(ligne)), Ok(None) à EOF, Err sinon.
fn io_read_line() -> Result<Value, RuntimeError> {
    use std::io::BufRead;
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) => Ok(make_ok(make_none())),   // EOF
        Ok(_) => {
            // Retire un seul terminateur de ligne : '\n', et le '\r' qui le
            // précède (cas '\r\n'). On ne touche pas aux autres '\r' qui font
            // partie de la donnée. read_line ne lit qu'une ligne, donc il n'y a
            // au plus qu'un '\n' (le dernier caractère).
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') { line.pop(); }
            }
            Ok(make_ok(make_some(Value::Str(line))))
        }
        Err(e) => Ok(io_err("ReadFailed", Some(e.to_string()))),
    }
}

/// Lit tout le reste de stdin. `Result<string, IoError>`.
fn io_read_all() -> Result<Value, RuntimeError> {
    use std::io::Read;
    let mut buf = String::new();
    match std::io::stdin().read_to_string(&mut buf) {
        Ok(_)  => Ok(make_ok(Value::Str(buf))),
        Err(e) => Ok(io_err("ReadFailed", Some(e.to_string()))),
    }
}

/// Lit un caractère Unicode sur stdin (1 à 4 octets UTF-8).
/// `Result<Option<char>, IoError>` : Ok(Some(c)), Ok(None) à EOF, Err sinon.
fn io_read_char() -> Result<Value, RuntimeError> {
    use std::io::Read;
    let stdin = std::io::stdin();
    let mut lock = stdin.lock();
    let mut bytes = [0u8; 4];
    let mut len = 0usize;
    loop {
        let mut b = [0u8; 1];
        match lock.read(&mut b) {
            Ok(0) => {
                return if len == 0 {
                    Ok(make_ok(make_none()))   // EOF propre
                } else {
                    Ok(io_err("ReadFailed", Some("séquence UTF-8 incomplète".to_string())))
                };
            }
            Ok(_) => {
                bytes[len] = b[0];
                len += 1;
                if let Ok(s) = std::str::from_utf8(&bytes[..len]) {
                    if let Some(c) = s.chars().next() {
                        return Ok(make_ok(make_some(Value::Char(c))));
                    }
                }
                if len == 4 {
                    return Ok(io_err("ReadFailed", Some("séquence UTF-8 invalide".to_string())));
                }
            }
            Err(e) => return Ok(io_err("ReadFailed", Some(e.to_string()))),
        }
    }
}

// ── Hachage de valeurs ────────────────────────────────────────────────────────

/// Calcule un code de hachage entier pour toute valeur minilang.
/// Utilisé par les builtins hashCode() des types primitifs et de Pair.
pub fn val_hash(v: &Value) -> i64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    match v {
        Value::Int(n)   => *n,
        Value::Byte(b)  => *b as i64,
        Value::Bool(b)  => if *b { 1 } else { 0 },
        Value::Char(c)  => *c as i64,
        Value::Str(s)   => {
            let mut h = DefaultHasher::new();
            s.hash(&mut h);
            h.finish() as i64
        }
        Value::Float(f) => f.to_bits() as i64,
        _ => 0,
    }
}

// ── Opérateurs binaires ───────────────────────────────────────────────────────

fn promote(l: Value, r: Value) -> (Value, Value) {
    match (&l, &r) {
        (Value::Int(a), Value::Float(_)) => (Value::Float(*a as f64), r),
        (Value::Float(_), Value::Int(b)) => (l, Value::Float(*b as f64)),
        _ => (l, r),
    }
}

fn eval_binop(lv: Value, op: &BinOp, rv: Value) -> Result<Value, RuntimeError> {
    let (lv, rv) = promote(lv, rv);
    match op {
        BinOp::Add => match (&lv, &rv) {
            (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Str(a),   Value::Str(b))   => Ok(Value::Str(format!("{}{}", a, b))),
            _ => err!("+ non applicable à {} et {}", lv, rv),
        },
        BinOp::Sub => match (&lv, &rv) {
            (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            _ => err!("- non applicable"),
        },
        BinOp::Mul => match (&lv, &rv) {
            (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            _ => err!("* non applicable"),
        },
        BinOp::Div => match (&lv, &rv) {
            (Value::Int(a),   Value::Int(b))   => {
                if *b == 0 { return err!("Division par zéro"); }
                Ok(Value::Int(a / b))
            }
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            _ => err!("/ non applicable"),
        },
        BinOp::Mod => match (&lv, &rv) {
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 { return err!("Modulo par zéro"); }
                Ok(Value::Int(a % b))
            }
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a % b)),
            _ => err!("% non applicable"),
        },
        BinOp::Pow => match (&lv, &rv) {
            (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(i64::pow(*a, (*b).max(0) as u32))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.powf(*b))),
            _ => err!("** non applicable"),
        },
        BinOp::Lt  => cmp(&lv, &rv, |a,b| a< b, |a,b| a< b),
        BinOp::Le  => cmp(&lv, &rv, |a,b| a<=b, |a,b| a<=b),
        BinOp::Gt  => cmp(&lv, &rv, |a,b| a> b, |a,b| a> b),
        BinOp::Ge  => cmp(&lv, &rv, |a,b| a>=b, |a,b| a>=b),
        BinOp::Eq  => Ok(Value::Bool(val_eq(&lv, &rv))),
        BinOp::Ne  => Ok(Value::Bool(!val_eq(&lv, &rv))),
        BinOp::And => match (&lv, &rv) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
            _ => err!("&& requiert des bool"),
        },
        BinOp::Or  => match (&lv, &rv) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a || *b)),
            _ => err!("|| requiert des bool"),
        },
    }
}

fn cmp(l: &Value, r: &Value,
    fi: impl Fn(i64,i64)->bool, ff: impl Fn(f64,f64)->bool
) -> Result<Value, RuntimeError> {
    match (l, r) {
        (Value::Int(a),   Value::Int(b))   => Ok(Value::Bool(fi(*a,*b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(ff(*a,*b))),
        _ => err!("Comparaison non applicable"),
    }
}

fn val_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x),   Value::Int(y))   => x == y,
        (Value::Byte(x),  Value::Byte(y))  => x == y,
        (Value::Float(x), Value::Float(y)) => (x-y).abs() < 1e-12,
        (Value::Bool(x),  Value::Bool(y))  => x == y,
        (Value::Str(x),   Value::Str(y))   => x == y,
        (Value::Char(x),  Value::Char(y))  => x == y,
        (Value::Null,     Value::Null)     => true,
        (Value::Enum(a),  Value::Enum(b))  =>
            a.enum_name == b.enum_name && a.variant_name == b.variant_name,
        _ => false,
    }
}

// ── API de test ───────────────────────────────────────────────────────────────

pub fn run_source(src: &str) -> Result<i64, String> {
    use chumsky::Parser;
    let full = format!("{}\n{}", crate::STDLIB, src);
    let program = crate::parser::program_parser()
        .parse(full.as_str())
        .map_err(|e| e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n"))?;
    Interpreter::new(&program).run(&program).map_err(|e| e.to_string())
}

/// Exécute la source et retourne la valeur de retour + toutes les lignes imprimées.
/// Les lignes sont également affichées sur la console.
pub fn run_source_with_output(src: &str) -> Result<(i64, Vec<String>), String> {
    use chumsky::Parser;
    let full = format!("{}\n{}", crate::STDLIB, src);
    let program = crate::parser::program_parser()
        .parse(full.as_str())
        .map_err(|e| e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n"))?;
    let captured = Rc::new(RefCell::new(Vec::<String>::new()));
    let cap = captured.clone();
    let print_fn: Box<dyn FnMut(&str)> = Box::new(move |line: &str| {
        println!("{}", line);
        cap.borrow_mut().push(line.to_string());
    });
    let ret = Interpreter::new_with_print(&program, print_fn)
        .run(&program)
        .map_err(|e| e.to_string())?;
    Ok((ret, Rc::try_unwrap(captured).unwrap().into_inner()))
}
