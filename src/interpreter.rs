// ─────────────────────────────────────────────────────────────────────────────
//  Interpréteur – évalue l'AST et exécute le programme
// ─────────────────────────────────────────────────────────────────────────────

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use log::{debug, info, warn};

use crate::ast::*;

// ─────────────────────────────────────────────────────────────────────────────
//  Valeurs runtime
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ObjectData {
    pub class_name: String,
    pub fields:     HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Array(Vec<Value>),
    Object(Rc<RefCell<ObjectData>>),
    Null,
    Void,
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
                let items: Vec<String> = v.iter().map(|x| x.to_string()).collect();
                write!(f, "[{}]", items.join(", "))
            }
            Value::Object(o) => write!(f, "<{}>", o.borrow().class_name),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Erreur runtime
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct RuntimeError(pub String);

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RuntimeError: {}", self.0)
    }
}

macro_rules! err {
    ($($arg:tt)*) => { Err(RuntimeError(format!($($arg)*))) };
}

// ─────────────────────────────────────────────────────────────────────────────
//  Environnement (pile de scopes)
// ─────────────────────────────────────────────────────────────────────────────

pub struct Environment {
    scopes: Vec<HashMap<String, Value>>,
}

impl Environment {
    pub fn new() -> Self { Self { scopes: vec![HashMap::new()] } }

    pub fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 { self.scopes.pop(); }
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) { return Some(v.clone()); }
        }
        None
    }

    pub fn set(&mut self, name: String, value: Value) {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(&name) { scope.insert(name, value); return; }
        }
        self.scopes.last_mut().unwrap().insert(name, value);
    }

    pub fn declare(&mut self, name: String, value: Value) {
        self.scopes.last_mut().unwrap().insert(name, value);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Flux de contrôle
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
enum Flow {
    Next,
    BreakLoop,
    ContinueLoop,
    Return(Value),
}

// ─────────────────────────────────────────────────────────────────────────────
//  Interpréteur
// ─────────────────────────────────────────────────────────────────────────────

pub struct Interpreter {
    classes: HashMap<String, ClassDef>,
}

impl Interpreter {
    pub fn new(program: &Program) -> Self {
        let classes = program.classes.iter().map(|c| (c.name.clone(), c.clone())).collect();
        Self { classes }
    }

    pub fn run(&mut self, program: &Program) -> Result<i64, RuntimeError> {
        info!("▶ Exécution");
        let mut env = Environment::new();

        match self.exec_body(&program.main.body, &mut env, None)? {
            Flow::Return(Value::Int(n)) => { info!("✓ main() → {}", n); Ok(n) }
            Flow::Return(v) => { warn!("main() valeur non-int : {}", v); Ok(0) }
            Flow::Next | Flow::BreakLoop | Flow::ContinueLoop => { warn!("main() sans return"); Ok(0) }
        }
    }

    // ── Valeur par défaut ─────────────────────────────────────────────────────

    fn default_value(ty: &Type) -> Value {
        match ty {
            Type::Int              => Value::Int(0),
            Type::Bool             => Value::Bool(false),
            Type::Float | Type::Double => Value::Float(0.0),
            Type::Str              => Value::Str(String::new()),
            Type::Array(_)         => Value::Array(vec![]),
            _                      => Value::Null,
        }
    }

    // ── Instanciation ─────────────────────────────────────────────────────────

    fn instantiate(&self, class_name: &str) -> Result<Value, RuntimeError> {
        if !self.classes.contains_key(class_name) {
            return err!("Classe inconnue : '{}'", class_name);
        }
        let fields = self.all_fields(class_name)
            .iter()
            .map(|f| (f.name.clone(), Self::default_value(&f.ty)))
            .collect();
        debug!("new {}()", class_name);
        Ok(Value::Object(Rc::new(RefCell::new(ObjectData { class_name: class_name.to_string(), fields }))))
    }

    fn all_fields(&self, class_name: &str) -> Vec<Field> {
        let Some(class) = self.classes.get(class_name) else { return vec![]; };
        let mut fields = class.parent.as_deref()
            .map(|p| self.all_fields(p))
            .unwrap_or_default();
        for f in &class.fields {
            fields.retain(|pf: &Field| pf.name != f.name);
            fields.push(f.clone());
        }
        fields
    }

    // ── Résolution de méthode (héritage) ──────────────────────────────────────

    fn find_method(&self, class_name: &str, method_name: &str) -> Option<Method> {
        let class = self.classes.get(class_name)?;
        if let Some(m) = class.methods.iter().find(|m| m.name == method_name) {
            return Some(m.clone());
        }
        if let Some(parent) = &class.parent {
            return self.find_method(parent, method_name);
        }
        None
    }

    // ── Corps ─────────────────────────────────────────────────────────────────

    fn exec_body(
        &mut self,
        stmts: &[Stmt],
        env:   &mut Environment,
        this:  Option<Rc<RefCell<ObjectData>>>,
    ) -> Result<Flow, RuntimeError> {
        for stmt in stmts {
            match self.exec_stmt(stmt, env, this.clone())? {
                Flow::Next => {}
                other      => return Ok(other),
            }
        }
        Ok(Flow::Next)
    }

    // ── Instructions ──────────────────────────────────────────────────────────

    fn exec_stmt(
        &mut self,
        stmt: &Stmt,
        env:  &mut Environment,
        this: Option<Rc<RefCell<ObjectData>>>,
    ) -> Result<Flow, RuntimeError> {
        match stmt {
            // ── Déclaration ───────────────────────────────────────────────────
            Stmt::VarDecl { ty, name, init } => {
                let value = if let Some(expr) = init {
                    self.eval_expr(expr, env, this)?
                } else if let Type::UserDefined(class_name) = ty {
                    self.instantiate(class_name)?
                } else if let Type::Generic(class_name, _) = ty {
                    self.instantiate(class_name)?
                } else {
                    Self::default_value(ty)
                };
                debug!("  decl {} = {}", name, value);
                env.declare(name.clone(), value);
            }

            // ── Affectation ───────────────────────────────────────────────────
            Stmt::Assign { target, value } => {
                let val = self.eval_expr(value, env, this.clone())?;
                debug!("  {} = {}", target, val);
                // Si target est un champ de this, on l'y affecte
                let is_field = this.as_ref()
                    .map(|t| t.borrow().fields.contains_key(target))
                    .unwrap_or(false);
                if is_field {
                    this.unwrap().borrow_mut().fields.insert(target.clone(), val);
                } else {
                    env.set(target.clone(), val);
                }
            }

            // ── Affectation de champ ──────────────────────────────────────────
            Stmt::FieldAssign { object, field, value } => {
                let val = self.eval_expr(value, env, this.clone())?;

                let obj_rc = if object == "this" {
                    this.clone().ok_or_else(|| RuntimeError("'this' hors méthode".into()))?
                } else {
                    let obj_val = env.get(object)
                        .or_else(|| this.as_ref()
                            .and_then(|t| t.borrow().fields.get(object).cloned()))
                        .ok_or_else(|| RuntimeError(format!("Variable inconnue '{}'", object)))?;
                    match obj_val {
                        Value::Object(rc) => rc,
                        _ => return err!("'{}' n'est pas un objet", object),
                    }
                };

                debug!("  {}.{} = {}", object, field, val);
                obj_rc.borrow_mut().fields.insert(field.clone(), val);
            }

            // ── print ─────────────────────────────────────────────────────────
            Stmt::Print(args) => {
                let parts: Vec<String> = args
                    .iter()
                    .map(|e| self.eval_expr(e, env, this.clone()).map(|v| v.to_string()))
                    .collect::<Result<_, _>>()?;
                println!("{}", parts.join(" "));
            }

            // ── return ────────────────────────────────────────────────────────
            Stmt::Return(expr) => {
                let val = match expr {
                    Some(e) => self.eval_expr(e, env, this)?,
                    None    => Value::Void,
                };
                return Ok(Flow::Return(val));
            }

            // ── ExprStmt ──────────────────────────────────────────────────────
            Stmt::ExprStmt(expr) => { self.eval_expr(expr, env, this)?; }

            // ── if ────────────────────────────────────────────────────────────
            Stmt::If { condition, then_body, else_body } => {
                let cond = self.eval_expr(condition, env, this.clone())?;
                match cond {
                    Value::Bool(true) => {
                        env.push_scope();
                        let f = self.exec_body(then_body, env, this)?;
                        env.pop_scope();
                        if !matches!(f, Flow::Next) { return Ok(f); }
                    }
                    Value::Bool(false) => {
                        if let Some(eb) = else_body {
                            env.push_scope();
                            let f = self.exec_body(eb, env, this)?;
                            env.pop_scope();
                            if !matches!(f, Flow::Next) { return Ok(f); }
                        }
                    }
                    _ => return err!("Condition if doit être bool"),
                }
            }

            // ── while ─────────────────────────────────────────────────────────
            Stmt::While { condition, body } => {
                loop {
                    match self.eval_expr(condition, env, this.clone())? {
                        Value::Bool(false) => break,
                        Value::Bool(true)  => {}
                        _ => return err!("Condition while doit être bool"),
                    }
                    env.push_scope();
                    let f = self.exec_body(body, env, this.clone())?;
                    env.pop_scope();
                    match f {
                        Flow::BreakLoop    => break,
                        Flow::ContinueLoop => continue,
                        Flow::Return(v)    => return Ok(Flow::Return(v)),
                        Flow::Next         => {}
                    }
                }
            }

            // ── do-while ──────────────────────────────────────────────────────
            Stmt::DoWhile { body, condition } => {
                loop {
                    env.push_scope();
                    let f = self.exec_body(body, env, this.clone())?;
                    env.pop_scope();
                    match f {
                        Flow::BreakLoop    => break,
                        Flow::ContinueLoop => {}
                        Flow::Return(v)    => return Ok(Flow::Return(v)),
                        Flow::Next         => {}
                    }
                    match self.eval_expr(condition, env, this.clone())? {
                        Value::Bool(false) => break,
                        Value::Bool(true)  => {}
                        _ => return err!("Condition do-while doit être bool"),
                    }
                }
            }

            // ── for ───────────────────────────────────────────────────────────
            Stmt::For { init, condition, update, body } => {
                env.push_scope();

                if let Some(s) = init {
                    self.exec_stmt(s, env, this.clone())?;
                }

                'for_loop: loop {
                    if let Some(cond_expr) = condition {
                        match self.eval_expr(cond_expr, env, this.clone())? {
                            Value::Bool(false) => break,
                            Value::Bool(true)  => {}
                            _ => return err!("Condition for doit être bool"),
                        }
                    }

                    env.push_scope();
                    let f = self.exec_body(body, env, this.clone())?;
                    env.pop_scope();

                    match f {
                        Flow::BreakLoop  => break 'for_loop,
                        Flow::Return(v)  => { env.pop_scope(); return Ok(Flow::Return(v)); }
                        Flow::Next | Flow::ContinueLoop => {}
                    }

                    if let Some(upd) = update {
                        self.exec_stmt(upd, env, this.clone())?;
                    }
                }

                env.pop_scope();
            }

            Stmt::Break    => return Ok(Flow::BreakLoop),
            Stmt::Continue => return Ok(Flow::ContinueLoop),
        }

        Ok(Flow::Next)
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  Évaluation d'expression
    // ─────────────────────────────────────────────────────────────────────────

    fn eval_expr(
        &mut self,
        expr: &Expr,
        env:  &mut Environment,
        this: Option<Rc<RefCell<ObjectData>>>,
    ) -> Result<Value, RuntimeError> {
        match expr {
            Expr::IntLit(n)    => Ok(Value::Int(*n)),
            Expr::FloatLit(f)  => Ok(Value::Float(*f)),
            Expr::BoolLit(b)   => Ok(Value::Bool(*b)),
            Expr::StringLit(s) => Ok(Value::Str(s.clone())),

            // ── Identifiant ───────────────────────────────────────────────────
            Expr::Ident(name) => {
                if name == "this" {
                    return this.as_ref()
                        .map(|rc| Value::Object(rc.clone()))
                        .ok_or_else(|| RuntimeError("'this' hors méthode".into()));
                }
                if let Some(v) = env.get(name) { return Ok(v); }
                if let Some(obj) = &this {
                    if let Some(v) = obj.borrow().fields.get(name) { return Ok(v.clone()); }
                }
                err!("Variable inconnue '{}'", name)
            }

            // ── Opération unaire ──────────────────────────────────────────────
            Expr::UnaryOp { op, expr } => {
                let v = self.eval_expr(expr, env, this)?;
                match op {
                    UnaryOp::Neg => match v {
                        Value::Int(n)   => Ok(Value::Int(-n)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        _ => err!("Opérateur - non applicable"),
                    },
                    UnaryOp::Not => match v {
                        Value::Bool(b) => Ok(Value::Bool(!b)),
                        _ => err!("Opérateur ! non applicable"),
                    },
                }
            }

            // ── Opération binaire ─────────────────────────────────────────────
            Expr::BinOp { left, op, right } => {
                let lv = self.eval_expr(left, env, this.clone())?;
                let rv = self.eval_expr(right, env, this)?;
                eval_binop(lv, op, rv)
            }

            // ── Accès champ ───────────────────────────────────────────────────
            Expr::FieldAccess { object, field } => {
                match self.eval_expr(object, env, this)? {
                    Value::Object(rc) => rc.borrow().fields.get(field).cloned()
                        .ok_or_else(|| RuntimeError(format!("Champ inconnu '{}'", field))),
                    _ => err!("Accès champ sur non-objet"),
                }
            }

            // ── Appel de méthode ──────────────────────────────────────────────
            Expr::MethodCall { object, method, args } => {
                let obj_val = self.eval_expr(object, env, this.clone())?;
                let eval_args = args.iter()
                    .map(|a| self.eval_expr(a, env, this.clone()))
                    .collect::<Result<Vec<_>, _>>()?;

                match obj_val {
                    Value::Object(obj_rc) => {
                        let class_name = obj_rc.borrow().class_name.clone();
                        let m = self.find_method(&class_name, method)
                            .ok_or_else(|| RuntimeError(format!(
                                "Méthode inconnue '{}.{}()'", class_name, method
                            )))?;
                        self.call_method(&m, eval_args, obj_rc)
                    }
                    _ => err!("Appel de méthode sur non-objet"),
                }
            }

            // ── Appel de fonction libre ────────────────────────────────────────
            Expr::FunctionCall { name, args } => {
                let eval_args = args.iter()
                    .map(|a| self.eval_expr(a, env, this.clone()))
                    .collect::<Result<Vec<_>, _>>()?;

                if let Some(obj_rc) = this {
                    let class_name = obj_rc.borrow().class_name.clone();
                    if let Some(m) = self.find_method(&class_name, name) {
                        return self.call_method(&m, eval_args, obj_rc);
                    }
                }
                err!("Fonction inconnue '{}'", name)
            }

            // ── new ClassName<T>(args) ─────────────────────────────────────────
            Expr::New { class_name, args, .. } => {
                // Instanciation avec valeurs par défaut
                let obj = self.instantiate(class_name)?;
                let obj_rc = match &obj {
                    Value::Object(rc) => rc.clone(),
                    _ => unreachable!(),
                };

                // Cherche un constructeur avec le bon nombre de paramètres
                let constructors = self.classes.get(class_name)
                    .map(|c| c.constructors.clone())
                    .unwrap_or_default();

                if !constructors.is_empty() {
                    let ctor = constructors.iter()
                        .find(|c| c.params.len() == args.len())
                        .cloned()
                        .ok_or_else(|| RuntimeError(format!(
                            "Pas de constructeur à {} arg(s) pour '{}'",
                            args.len(), class_name
                        )))?;

                    let eval_args = args.iter()
                        .map(|a| self.eval_expr(a, env, None))
                        .collect::<Result<Vec<_>, _>>()?;

                    // Exécute le corps du constructeur avec this = nouvel objet
                    let mut ctor_env = Environment::new();
                    ctor_env.push_scope();
                    for (p, v) in ctor.params.iter().zip(eval_args) {
                        ctor_env.declare(p.name.clone(), v);
                    }
                    self.exec_body(&ctor.body, &mut ctor_env, Some(obj_rc))?;
                }

                Ok(obj)
            }
        }
    }

    // ── Appel de méthode ──────────────────────────────────────────────────────

    fn call_method(
        &mut self,
        method: &Method,
        args:   Vec<Value>,
        this:   Rc<RefCell<ObjectData>>,
    ) -> Result<Value, RuntimeError> {
        if args.len() != method.params.len() {
            return err!("'{}()' : {} arg(s) attendus, {} fournis",
                method.name, method.params.len(), args.len());
        }
        debug!("→ {}", method.name);

        let mut env = Environment::new();
        env.push_scope();
        for (p, v) in method.params.iter().zip(args) {
            env.declare(p.name.clone(), v);
        }

        match self.exec_body(&method.body.clone(), &mut env, Some(this))? {
            Flow::Return(v) => Ok(v),
            _               => Ok(Value::Void),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Évaluation des opérateurs binaires
// ─────────────────────────────────────────────────────────────────────────────

fn eval_binop(lv: Value, op: &BinOp, rv: Value) -> Result<Value, RuntimeError> {
    // Promotions numériques
    let (lv, rv) = promote(lv, rv);

    match op {
        BinOp::Add => match (lv, rv) {
            (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Str(a),   Value::Str(b))   => Ok(Value::Str(a + &b)),
            (l, r) => err!("+ non applicable à {} et {}", l, r),
        },
        BinOp::Sub => match (lv, rv) {
            (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (l, r) => err!("- non applicable à {} et {}", l, r),
        },
        BinOp::Mul => match (lv, rv) {
            (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (l, r) => err!("* non applicable à {} et {}", l, r),
        },
        BinOp::Div => match (lv, rv) {
            (Value::Int(a),   Value::Int(b))   => {
                if b == 0 { return err!("Division par zéro"); }
                Ok(Value::Int(a / b))
            }
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            (l, r) => err!("/ non applicable à {} et {}", l, r),
        },
        BinOp::Mod => match (lv, rv) {
            (Value::Int(a),   Value::Int(b))   => {
                if b == 0 { return err!("Modulo par zéro"); }
                Ok(Value::Int(a % b))
            }
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a % b)),
            (l, r) => err!("% non applicable à {} et {}", l, r),
        },
        BinOp::Pow => match (lv, rv) {
            (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(i64::pow(a, b.max(0) as u32))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.powf(b))),
            (l, r) => err!("** non applicable à {} et {}", l, r),
        },
        BinOp::Lt  => cmp_op(lv, rv, |a, b| a <  b, |a, b| a <  b),
        BinOp::Le  => cmp_op(lv, rv, |a, b| a <= b, |a, b| a <= b),
        BinOp::Gt  => cmp_op(lv, rv, |a, b| a >  b, |a, b| a >  b),
        BinOp::Ge  => cmp_op(lv, rv, |a, b| a >= b, |a, b| a >= b),
        BinOp::Eq  => Ok(Value::Bool(values_equal(&lv, &rv))),
        BinOp::Ne  => Ok(Value::Bool(!values_equal(&lv, &rv))),
        BinOp::And => match (lv, rv) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
            _ => err!("&& requiert des bool"),
        },
        BinOp::Or  => match (lv, rv) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
            _ => err!("|| requiert des bool"),
        },
    }
}

/// Promotion int → float si l'un des deux est float
fn promote(lv: Value, rv: Value) -> (Value, Value) {
    match (&lv, &rv) {
        (Value::Int(a), Value::Float(_)) => (Value::Float(*a as f64), rv),
        (Value::Float(_), Value::Int(b)) => (lv, Value::Float(*b as f64)),
        _ => (lv, rv),
    }
}

fn cmp_op(
    lv: Value, rv: Value,
    fi: impl Fn(i64, i64) -> bool,
    ff: impl Fn(f64, f64) -> bool,
) -> Result<Value, RuntimeError> {
    match (lv, rv) {
        (Value::Int(a),   Value::Int(b))   => Ok(Value::Bool(fi(a, b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(ff(a, b))),
        (l, r) => err!("Comparaison non applicable à {} et {}", l, r),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x),   Value::Int(y))   => x == y,
        (Value::Float(x), Value::Float(y)) => (x - y).abs() < 1e-12,
        (Value::Bool(x),  Value::Bool(y))  => x == y,
        (Value::Str(x),   Value::Str(y))   => x == y,
        (Value::Null,     Value::Null)     => true,
        _ => false,
    }
}
