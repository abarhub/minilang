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
pub struct ObjectData {
    pub class_name: String,
    pub fields:     HashMap<String, Value>,
}

/// Valeur runtime d'une variante d'enum
#[derive(Debug, Clone)]
pub struct EnumData {
    pub enum_name:    String,
    pub variant_name: String,
    /// Champs nommés de la variante
    pub fields:       HashMap<String, Value>,
    /// Ordre d'insertion (pour les patterns positionnels)
    pub field_order:  Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64), Float(f64), Bool(bool), Str(String),
    Array(Vec<Value>),
    Object(Rc<RefCell<ObjectData>>),
    /// Valeur d'un enum : EnumName::Variant { champs }
    Enum(Rc<EnumData>),
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
                let items: Vec<String> = v.iter().map(|x| x.to_string()).collect();
                write!(f, "[{}]", items.join(", "))
            }
            Value::Object(o) => write!(f, "<{}>", o.borrow().class_name),
            Value::Enum(e)   => {
                if e.field_order.is_empty() {
                    write!(f, "{}::{}", e.enum_name, e.variant_name)
                } else {
                    let vals: Vec<String> = e.field_order.iter()
                        .map(|k| e.fields[k].to_string()).collect();
                    write!(f, "{}::{}({})", e.enum_name, e.variant_name, vals.join(", "))
                }
            }
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

macro_rules! err {
    ($($arg:tt)*) => { Err(RuntimeError(format!($($arg)*))) };
}

// ── Environnement ─────────────────────────────────────────────────────────────

pub struct Environment {
    scopes: Vec<HashMap<String, Value>>,
}

impl Environment {
    pub fn new() -> Self { Self { scopes: vec![HashMap::new()] } }
    pub fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }
    pub fn pop_scope(&mut self) { if self.scopes.len() > 1 { self.scopes.pop(); } }

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

// ── Flux de contrôle ─────────────────────────────────────────────────────────

#[derive(Debug)]
enum Flow {
    Next, BreakLoop, ContinueLoop, Return(Value),
}

// ── Interpréteur ─────────────────────────────────────────────────────────────

pub struct Interpreter {
    classes: HashMap<String, ClassDef>,
    enums:   HashMap<String, EnumDef>,
}

impl Interpreter {
    pub fn new(program: &Program) -> Self {
        Self {
            classes: program.classes.iter().map(|c| (c.name.clone(), c.clone())).collect(),
            enums:   program.enums.iter().map(|e| (e.name.clone(), e.clone())).collect(),
        }
    }

    pub fn run(&mut self, program: &Program) -> Result<i64, RuntimeError> {
        info!("▶ Exécution");
        let mut env = Environment::new();
        match self.exec_body(&program.main.body, &mut env, None)? {
            Flow::Return(Value::Int(n)) => { info!("✓ main() → {}", n); Ok(n) }
            Flow::Return(v) => { warn!("main() valeur non-int : {}", v); Ok(0) }
            _ => { warn!("main() sans return"); Ok(0) }
        }
    }

    // ── Valeurs par défaut ────────────────────────────────────────────────────

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

    // ── Instanciation de classe ───────────────────────────────────────────────

    fn instantiate(&self, class_name: &str) -> Result<Value, RuntimeError> {
        if !self.classes.contains_key(class_name) {
            return err!("Classe inconnue : '{}'", class_name);
        }
        let fields = self.all_fields(class_name).iter()
            .map(|f| (f.name.clone(), Self::default_value(&f.ty))).collect();
        debug!("new {}()", class_name);
        Ok(Value::Object(Rc::new(RefCell::new(ObjectData {
            class_name: class_name.to_string(), fields
        }))))
    }

    fn all_fields(&self, class_name: &str) -> Vec<Field> {
        let Some(class) = self.classes.get(class_name) else { return vec![]; };
        let mut fields = class.parent.as_deref()
            .map(|p| self.all_fields(p)).unwrap_or_default();
        for f in &class.fields {
            fields.retain(|pf: &Field| pf.name != f.name);
            fields.push(f.clone());
        }
        fields
    }

    // ── Résolution de méthode (héritage + enum) ───────────────────────────────

    fn find_method(&self, class_name: &str, method_name: &str) -> Option<Method> {
        // Classes
        if let Some(class) = self.classes.get(class_name) {
            if let Some(m) = class.methods.iter().find(|m| m.name == method_name) {
                return Some(m.clone());
            }
            if let Some(parent) = &class.parent {
                return self.find_method(parent, method_name);
            }
        }
        // Enums
        if let Some(ed) = self.enums.get(class_name) {
            if let Some(m) = ed.methods.iter().find(|m| m.name == method_name) {
                return Some(m.clone());
            }
        }
        None
    }

    // ── Corps ─────────────────────────────────────────────────────────────────

    fn exec_body(
        &mut self, stmts: &[Stmt], env: &mut Environment,
        this: Option<Rc<RefCell<ObjectData>>>,
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
        &mut self, stmt: &Stmt, env: &mut Environment,
        this: Option<Rc<RefCell<ObjectData>>>,
    ) -> Result<Flow, RuntimeError> {
        match stmt {
            Stmt::VarDecl { ty, name, init } => {
                let value = if let Some(expr) = init {
                    self.eval_expr(expr, env, this)?
                } else if let Type::UserDefined(cn) = ty {
                    if self.enums.contains_key(cn) { Value::Null }
                    else { self.instantiate(cn)? }
                } else if let Type::Generic(cn, _) = ty {
                    self.instantiate(cn)?
                } else {
                    Self::default_value(ty)
                };
                debug!("  decl {} = {}", name, value);
                env.declare(name.clone(), value);
            }

            Stmt::Assign { target, value } => {
                let val = self.eval_expr(value, env, this.clone())?;
                let is_field = this.as_ref()
                    .map(|t| t.borrow().fields.contains_key(target))
                    .unwrap_or(false);
                if is_field {
                    this.unwrap().borrow_mut().fields.insert(target.clone(), val);
                } else {
                    env.set(target.clone(), val);
                }
            }

            Stmt::FieldAssign { object, field, value } => {
                let val = self.eval_expr(value, env, this.clone())?;
                let obj_rc = if object == "this" {
                    this.clone().ok_or_else(|| RuntimeError("'this' hors méthode".into()))?
                } else {
                    let obj_val = env.get(object)
                        .or_else(|| this.as_ref().and_then(|t| t.borrow().fields.get(object).cloned()))
                        .ok_or_else(|| RuntimeError(format!("Variable inconnue '{}'", object)))?;
                    match obj_val {
                        Value::Object(rc) => rc,
                        _ => return err!("'{}' n'est pas un objet", object),
                    }
                };
                obj_rc.borrow_mut().fields.insert(field.clone(), val);
            }

            Stmt::Print(args) => {
                let parts: Vec<String> = args.iter()
                    .map(|e| self.eval_expr(e, env, this.clone()).map(|v| v.to_string()))
                    .collect::<Result<_, _>>()?;
                println!("{}", parts.join(" "));
            }

            Stmt::Return(expr) => {
                let val = match expr {
                    Some(e) => self.eval_expr(e, env, this)?,
                    None    => Value::Void,
                };
                return Ok(Flow::Return(val));
            }

            Stmt::ExprStmt(expr) => { self.eval_expr(expr, env, this)?; }

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

            Stmt::DoWhile { body, condition } => {
                loop {
                    env.push_scope();
                    let f = self.exec_body(body, env, this.clone())?;
                    env.pop_scope();
                    match f {
                        Flow::BreakLoop    => break,
                        Flow::Return(v)    => return Ok(Flow::Return(v)),
                        Flow::ContinueLoop | Flow::Next => {}
                    }
                    match self.eval_expr(condition, env, this.clone())? {
                        Value::Bool(false) => break,
                        Value::Bool(true)  => {}
                        _ => return err!("Condition do-while doit être bool"),
                    }
                }
            }

            Stmt::For { init, condition, update, body } => {
                env.push_scope();
                if let Some(s) = init { self.exec_stmt(s, env, this.clone())?; }
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
                    if let Some(upd) = update { self.exec_stmt(upd, env, this.clone())?; }
                }
                env.pop_scope();
            }

            Stmt::Break    => return Ok(Flow::BreakLoop),
            Stmt::Continue => return Ok(Flow::ContinueLoop),

            // ── match ─────────────────────────────────────────────────────────
            Stmt::Match { expr, arms } => {
                let val = self.eval_expr(expr, env, this.clone())?;

                for arm in arms {
                    let matched = match &arm.pattern {
                        Pattern::Wildcard => true,

                        Pattern::Variant { name: variant_name, bindings } => {
                            match &val {
                                Value::Enum(ed) => {
                                    if ed.variant_name == *variant_name {
                                        // Lie les bindings dans un scope frais
                                        env.push_scope();
                                        for (binding, field_name) in
                                            bindings.iter().zip(ed.field_order.iter())
                                        {
                                            let field_val = ed.fields.get(field_name)
                                                .cloned().unwrap_or(Value::Null);
                                            env.declare(binding.clone(), field_val);
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                }
                                _ => false,
                            }
                        }
                    };

                    if matched {
                        // Si le pattern a créé un scope (Variant), le corps s'exécute dedans
                        let need_pop = matches!(&arm.pattern, Pattern::Variant { .. })
                            && matches!(&val, Value::Enum(_));

                        if !need_pop { env.push_scope(); }
                        let f = self.exec_body(&arm.body, env, this.clone())?;
                        env.pop_scope();

                        match f {
                            Flow::Next => {}
                            other      => return Ok(other),
                        }
                        break; // un seul bras exécuté
                    }
                }
            }
        }

        Ok(Flow::Next)
    }

    // ── Évaluation d'expression ───────────────────────────────────────────────

    fn eval_expr(
        &mut self, expr: &Expr, env: &mut Environment,
        this: Option<Rc<RefCell<ObjectData>>>,
    ) -> Result<Value, RuntimeError> {
        match expr {
            Expr::IntLit(n)    => Ok(Value::Int(*n)),
            Expr::FloatLit(f)  => Ok(Value::Float(*f)),
            Expr::BoolLit(b)   => Ok(Value::Bool(*b)),
            Expr::StringLit(s) => Ok(Value::Str(s.clone())),

            Expr::Ident(name) => {
                // L'env est consulté EN PREMIER : cela permet aux méthodes d'enum
                // de stocker `this = Value::Enum(...)` dans l'environnement et d'avoir
                // la priorité sur le `this: Option<Rc<RefCell<ObjectData>>>` des classes.
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

            Expr::BinOp { left, op, right } => {
                let lv = self.eval_expr(left, env, this.clone())?;
                let rv = self.eval_expr(right, env, this)?;
                eval_binop(lv, op, rv)
            }

            Expr::FieldAccess { object, field } => {
                match self.eval_expr(object, env, this)? {
                    Value::Object(rc) => rc.borrow().fields.get(field).cloned()
                        .ok_or_else(|| RuntimeError(format!("Champ inconnu '{}'", field))),
                    Value::Enum(ed)   => ed.fields.get(field).cloned()
                        .ok_or_else(|| RuntimeError(format!("Champ inconnu '{}' dans variante '{}'",
                            field, ed.variant_name))),
                    _ => err!("Accès champ sur non-objet"),
                }
            }

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
                                "Méthode inconnue '{}.{}()'", class_name, method)))?;
                        self.call_method(&m, eval_args, obj_rc)
                    }
                    Value::Enum(ed) => {
                        let enum_name = ed.enum_name.clone();
                        let m = self.find_method(&enum_name, method)
                            .ok_or_else(|| RuntimeError(format!(
                                "Méthode inconnue '{}::{}()'", enum_name, method)))?;
                        self.call_enum_method(&m, eval_args, ed)
                    }
                    _ => err!("Appel de méthode sur non-objet"),
                }
            }

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

            Expr::New { class_name, args, .. } => {
                let obj = self.instantiate(class_name)?;
                let obj_rc = match &obj {
                    Value::Object(rc) => rc.clone(),
                    _ => unreachable!(),
                };
                let constructors = self.classes.get(class_name)
                    .map(|c| c.constructors.clone()).unwrap_or_default();
                if !constructors.is_empty() {
                    let ctor = constructors.iter()
                        .find(|c| c.params.len() == args.len()).cloned()
                        .ok_or_else(|| RuntimeError(format!(
                            "Pas de constructeur à {} arg(s) pour '{}'", args.len(), class_name)))?;
                    let eval_args = args.iter()
                        .map(|a| self.eval_expr(a, env, None))
                        .collect::<Result<Vec<_>, _>>()?;
                    let mut ctor_env = Environment::new();
                    ctor_env.push_scope();
                    for (p, v) in ctor.params.iter().zip(eval_args) {
                        ctor_env.declare(p.name.clone(), v);
                    }
                    self.exec_body(&ctor.body, &mut ctor_env, Some(obj_rc))?;
                }
                Ok(obj)
            }

            // ── Constructeur d'enum : EnumName::Variant(args) ─────────────────
            Expr::EnumConstructor { enum_name, variant, args } => {
                let enum_def = self.enums.get(enum_name)
                    .ok_or_else(|| RuntimeError(format!("Enum inconnu '{}'", enum_name)))?
                    .clone();
                let var_def = enum_def.variants.iter()
                    .find(|v| &v.name == variant)
                    .ok_or_else(|| RuntimeError(format!(
                        "Variante '{}' inconnue dans '{}'", variant, enum_name)))?
                    .clone();

                let eval_args = args.iter()
                    .map(|a| self.eval_expr(a, env, None))
                    .collect::<Result<Vec<_>, _>>()?;

                let mut fields      = HashMap::new();
                let mut field_order = Vec::new();
                for (param, val) in var_def.fields.iter().zip(eval_args) {
                    fields.insert(param.name.clone(), val);
                    field_order.push(param.name.clone());
                }

                debug!("{}::{}", enum_name, variant);
                Ok(Value::Enum(Rc::new(EnumData {
                    enum_name:    enum_name.clone(),
                    variant_name: variant.clone(),
                    fields,
                    field_order,
                })))
            }
        }
    }

    // ── Appel de méthode ─────────────────────────────────────────────────────

    fn call_method(
        &mut self, method: &Method, args: Vec<Value>, this: Rc<RefCell<ObjectData>>,
    ) -> Result<Value, RuntimeError> {
        if args.len() != method.params.len() {
            return err!("'{}()' : {} arg(s) attendus, {} fournis",
                method.name, method.params.len(), args.len());
        }
        debug!("→ {}", method.name);
        let mut env = Environment::new();
        env.push_scope();
        for (p, v) in method.params.iter().zip(args) { env.declare(p.name.clone(), v); }
        match self.exec_body(&method.body.clone(), &mut env, Some(this))? {
            Flow::Return(v) => Ok(v),
            _               => Ok(Value::Void),
        }
    }

    // ── Appel d'une méthode d'enum ────────────────────────────────────────────
    //
    // Contrairement à `call_method`, on ne passe pas d'`ObjectData` comme `this`.
    // On stocke à la place `Value::Enum(ed)` directement dans l'environnement sous
    // la clé `"this"`.  Grâce à la modification de `Expr::Ident` qui consulte l'env
    // en premier, `this` s'évalue alors en `Value::Enum` — ce qui permet au `match`
    // du corps de la méthode de fonctionner correctement.

    fn call_enum_method(
        &mut self, method: &Method, args: Vec<Value>, ed: Rc<EnumData>,
    ) -> Result<Value, RuntimeError> {
        if args.len() != method.params.len() {
            return err!("'{}()' : {} arg(s) attendus, {} fournis",
                method.name, method.params.len(), args.len());
        }
        debug!("→ enum::{}", method.name);
        let mut env = Environment::new();
        env.push_scope();
        // `this` = la valeur enum elle-même, accessible via env
        env.declare("this".to_string(), Value::Enum(ed));
        for (p, v) in method.params.iter().zip(args) { env.declare(p.name.clone(), v); }
        // On passe `this = None` : l'ObjectData n'est pas utilisé ici
        match self.exec_body(&method.body.clone(), &mut env, None)? {
            Flow::Return(v) => Ok(v),
            _               => Ok(Value::Void),
        }
    }
}

// ── Opérateurs binaires ───────────────────────────────────────────────────────

fn eval_binop(lv: Value, op: &BinOp, rv: Value) -> Result<Value, RuntimeError> {
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
        .map_err(|errs| errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))?;
    let mut interp = Interpreter::new(&program);
    interp.run(&program).map_err(|e| e.to_string())
}
