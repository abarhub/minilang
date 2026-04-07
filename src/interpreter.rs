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
    Int(i64), Float(f64), Bool(bool), Str(String),
    Array(Vec<Value>),
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
            Value::Float(n)  => write!(f, "{}", n),
            Value::Bool(b)   => write!(f, "{}", b),
            Value::Str(s)    => write!(f, "{}", s),
            Value::Null      => write!(f, "null"),
            Value::Void      => write!(f, ""),
            Value::Array(v)  => {
                write!(f, "[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "))
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
    classes: HashMap<String, ClassDef>,
    enums:   HashMap<String, EnumDef>,
}

impl Interpreter {
    pub fn new(program: &Program) -> Self {
        Self {
            classes: program.classes.iter().map(|c| (c.name.clone(), c.clone())).collect(),
            enums:   program.enums  .iter().map(|e| (e.name.clone(), e.clone())).collect(),
        }
    }

    pub fn run(&mut self, program: &Program) -> Result<i64, RuntimeError> {
        info!("▶ Exécution");
        let mut env = Env::new();
        match self.exec_body(&program.main.body, &mut env, None)? {
            Flow::Return(Value::Int(n)) => { info!("✓ main → {}", n); Ok(n) }
            Flow::Return(v) => { warn!("main valeur non-int : {}", v); Ok(0) }
            _ => { warn!("main sans return"); Ok(0) }
        }
    }

    // ── Valeur par défaut ─────────────────────────────────────────────────────

    fn default_value(ty: &Type) -> Value {
        match ty {
            Type::Int            => Value::Int(0),
            Type::Bool           => Value::Bool(false),
            Type::Float | Type::Double => Value::Float(0.0),
            Type::Str            => Value::Str(String::new()),
            Type::Array(_)       => Value::Array(vec![]),
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
            Stmt::VarDecl { ty, name, init } => {
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
                println!("{}", parts.join(" "));
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
                        self.call_method(&m, args, rc)
                    }
                    Value::Enum(ed) => {
                        let en = ed.enum_name.clone();
                        let m = self.find_method(&en, method)
                            .ok_or_else(|| RuntimeError(format!("Méthode inconnue '{}::{}()'", en, method)))?;
                        self.call_enum_method(&m, args, ed)
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
                err!("Fonction inconnue '{}'", name)
            }

            Expr::New { class_name, args, .. } => {
                let obj = self.instantiate(class_name)?;
                let rc = match &obj { Value::Object(r) => r.clone(), _ => unreachable!() };
                let ctors = self.classes.get(class_name)
                    .map(|c| c.constructors.clone()).unwrap_or_default();
                if !ctors.is_empty() {
                    let ctor = ctors.iter().find(|c| c.params.len() == args.len()).cloned()
                        .ok_or_else(|| RuntimeError(format!(
                            "Pas de constructeur à {} arg(s) pour '{}'", args.len(), class_name)))?;
                    let eargs: Vec<Value> = args.iter()
                        .map(|a| self.eval(a, env, None)).collect::<Result<_, _>>()?;
                    let mut ce = Env::new(); ce.push();
                    for (p, v) in ctor.params.iter().zip(eargs) { ce.declare(p.name.clone(), v); }
                    self.exec_body(&ctor.body, &mut ce, Some(rc))?;
                }
                Ok(obj)
            }

            Expr::EnumConstructor { enum_name, variant, args } => {
                let ed = self.enums.get(enum_name)
                    .ok_or_else(|| RuntimeError(format!("Enum inconnu '{}'", enum_name)))?.clone();
                let vd = ed.variants.iter().find(|v| &v.name == variant)
                    .ok_or_else(|| RuntimeError(format!("Variante '{}' inconnue", variant)))?.clone();
                let eargs: Vec<Value> = args.iter()
                    .map(|a| self.eval(a, env, None)).collect::<Result<_, _>>()?;
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
        }
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
        (Value::Float(x), Value::Float(y)) => (x-y).abs() < 1e-12,
        (Value::Bool(x),  Value::Bool(y))  => x == y,
        (Value::Str(x),   Value::Str(y))   => x == y,
        (Value::Null,     Value::Null)     => true,
        (Value::Enum(a),  Value::Enum(b))  =>
            a.enum_name == b.enum_name && a.variant_name == b.variant_name,
        _ => false,
    }
}

// ── API de test ───────────────────────────────────────────────────────────────

pub fn run_source(src: &str) -> Result<i64, String> {
    use chumsky::Parser;
    let program = crate::parser::program_parser()
        .parse(src)
        .map_err(|e| e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n"))?;
    Interpreter::new(&program).run(&program).map_err(|e| e.to_string())
}
