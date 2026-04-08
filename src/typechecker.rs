// ─────────────────────────────────────────────────────────────────────────────
//  Typechecker
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::{HashMap, HashSet};
use log::{debug, warn};
use crate::ast::*;

// ── Erreur ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TypeError(pub String);

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypeError: {}", self.0)
    }
}

macro_rules! type_err { ($($a:tt)*) => { Err(TypeError(format!($($a)*))) }; }

// ── Environnement de types ────────────────────────────────────────────────────

struct TypeEnv {
    scopes: Vec<HashMap<String, Type>>,
}

impl TypeEnv {
    fn new() -> Self { Self { scopes: vec![HashMap::new()] } }
    fn push(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop(&mut self)  { if self.scopes.len() > 1 { self.scopes.pop(); } }

    fn declare(&mut self, name: String, ty: Type) {
        self.scopes.last_mut().unwrap().insert(name, ty);
    }
    fn get(&self, name: &str) -> Option<&Type> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }
    fn set(&mut self, name: &str, ty: Type) {
        for s in self.scopes.iter_mut().rev() {
            if s.contains_key(name) { s.insert(name.to_string(), ty); return; }
        }
        self.scopes.last_mut().unwrap().insert(name.to_string(), ty);
    }

    /// Instantané de toutes les variables visibles (pour la capture des lambdas)
    fn snapshot(&self) -> Vec<(String, Type)> {
        let mut result: HashMap<String, Type> = HashMap::new();
        for scope in &self.scopes {
            for (k, v) in scope { result.insert(k.clone(), v.clone()); }
        }
        result.into_iter().collect()
    }
}

// ── Substitution de paramètres génériques ─────────────────────────────────────

fn substitute(ty: &Type, subst: &[(String, Type)]) -> Type {
    match ty {
        Type::UserDefined(n) => subst.iter()
            .find(|(k, _)| k == n).map(|(_, v)| v.clone())
            .unwrap_or_else(|| ty.clone()),
        Type::Array(i) => Type::Array(Box::new(substitute(i, subst))),
        Type::Generic(n, args) =>
            Type::Generic(n.clone(), args.iter().map(|a| substitute(a, subst)).collect()),
        Type::FnType(params, ret) => Type::FnType(
            params.iter().map(|p| substitute(p, subst)).collect(),
            Box::new(substitute(ret, subst)),
        ),
        _ => ty.clone(),
    }
}

// ── TypeChecker ───────────────────────────────────────────────────────────────

pub struct TypeChecker {
    classes:         HashMap<String, ClassDef>,
    interfaces:      HashMap<String, InterfaceDef>,
    enums:           HashMap<String, EnumDef>,
    aliases:         HashMap<String, Type>,    // ← résolution des alias
    type_params:     HashSet<String>,
    current_class:   Option<String>,
    current_enum:    Option<String>,
    expected_return: Type,
    errors:          Vec<TypeError>,
}

impl TypeChecker {
    pub fn new(program: &Program) -> Self {
        Self {
            classes:         program.classes.iter().map(|c| (c.name.clone(), c.clone())).collect(),
            interfaces:      program.interfaces.iter().map(|i| (i.name.clone(), i.clone())).collect(),
            enums:           program.enums.iter().map(|e| (e.name.clone(), e.clone())).collect(),
            aliases:         program.type_aliases.iter().map(|a| (a.name.clone(), a.ty.clone())).collect(),
            type_params:     HashSet::new(),
            current_class:   None,
            current_enum:    None,
            expected_return: Type::Void,
            errors:          vec![],
        }
    }

    pub fn check(mut self, program: &Program) -> Vec<TypeError> {
        self.check_program(program);
        self.errors
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn err(&mut self, msg: String) { self.errors.push(TypeError(msg)); }

    fn infer(&mut self, expr: &Expr, env: &TypeEnv) -> Option<Type> {
        match self.infer_expr(expr, env) {
            Ok(t)  => Some(t),
            Err(e) => { self.errors.push(e); None }
        }
    }

    // ── Résolution d'alias ────────────────────────────────────────────────────
    //
    //  Transforme récursivement `UserDefined("Adder")` → `FnType([Int], Int)`
    //  si "Adder" est un alias déclaré.  Gère les alias imbriqués.

    fn resolve(&self, ty: &Type) -> Type {
        match ty {
            Type::UserDefined(n) => {
                if let Some(aliased) = self.aliases.get(n) {
                    self.resolve(aliased)
                } else {
                    ty.clone()
                }
            }
            Type::Array(inner)       => Type::Array(Box::new(self.resolve(inner))),
            Type::FnType(params, ret) => Type::FnType(
                params.iter().map(|p| self.resolve(p)).collect(),
                Box::new(self.resolve(ret)),
            ),
            Type::Generic(n, args)   => Type::Generic(
                n.clone(), args.iter().map(|a| self.resolve(a)).collect(),
            ),
            _ => ty.clone(),
        }
    }

    // ── Programme ─────────────────────────────────────────────────────────────

    fn check_program(&mut self, program: &Program) {
        self.check_class_hierarchy();
        self.check_interface_impls();
        self.check_enums(program);
        for class in &program.classes.clone() { self.check_class(class); }
        let mut env = TypeEnv::new();
        self.expected_return = Type::Int;
        for stmt in &program.main.body.clone() { self.check_stmt(stmt, &mut env); }
    }

    // ── Hiérarchie ────────────────────────────────────────────────────────────

    fn check_class_hierarchy(&mut self) {
        for name in &self.classes.keys().cloned().collect::<Vec<_>>() {
            let class = self.classes[name].clone();
            if let Some(p) = &class.parent {
                if !self.classes.contains_key(p) {
                    self.err(format!("Classe '{}' extends '{}' inconnu", name, p));
                }
            }
            for iface in &class.implements {
                if !self.interfaces.contains_key(iface) {
                    self.err(format!("Classe '{}' implements '{}' inconnu", name, iface));
                }
            }
            if self.has_cycle(name) {
                self.err(format!("Cycle d'héritage pour '{}'", name));
            }
        }
    }

    fn has_cycle(&self, start: &str) -> bool {
        let mut visited = HashSet::new();
        let mut cur = start.to_string();
        loop {
            if visited.contains(&cur) { return true; }
            visited.insert(cur.clone());
            match self.classes.get(&cur).and_then(|c| c.parent.clone()) {
                Some(p) => cur = p,
                None    => return false,
            }
        }
    }

    fn check_interface_impls(&mut self) {
        for cn in &self.classes.keys().cloned().collect::<Vec<_>>() {
            let class = self.classes[cn].clone();
            for iname in &class.implements.clone() {
                if let Some(iface) = self.interfaces.get(iname).cloned() {
                    for sig in &iface.methods {
                        if self.find_method_def(cn, &sig.name).is_none() {
                            self.err(format!("'{}' n'implémente pas '{}.{}()'", cn, iname, sig.name));
                        }
                    }
                }
            }
        }
    }

    // ── Enums ─────────────────────────────────────────────────────────────────

    fn check_enums(&mut self, program: &Program) {
        for ed in &program.enums.clone() {
            self.current_enum = Some(ed.name.clone());
            let mut seen = HashSet::new();
            for v in &ed.variants {
                if !seen.insert(v.name.clone()) {
                    self.err(format!("Enum '{}' : variante '{}' dupliquée", ed.name, v.name));
                }
            }
            for m in &ed.methods.clone() {
                let mut env = TypeEnv::new(); env.push();
                for p in &m.params { env.declare(p.name.clone(), self.resolve(&p.ty)); }
                self.expected_return = self.resolve(&m.return_type);
                for s in &m.body.clone() { self.check_stmt(s, &mut env); }
            }
            self.current_enum = None;
        }
    }

    // ── Classe ────────────────────────────────────────────────────────────────

    fn check_class(&mut self, class: &ClassDef) {
        self.current_class = Some(class.name.clone());
        self.type_params   = class.type_params.iter().cloned().collect();
        debug!("check class '{}'", class.name);
        for ctor in &class.constructors.clone() {
            let mut env = TypeEnv::new(); env.push();
            for p in &ctor.params { env.declare(p.name.clone(), self.resolve(&p.ty)); }
            self.expected_return = Type::Void;
            for s in &ctor.body.clone() { self.check_stmt(s, &mut env); }
        }
        for m in &class.methods.clone() {
            let mut env = TypeEnv::new(); env.push();
            for p in &m.params { env.declare(p.name.clone(), self.resolve(&p.ty)); }
            self.expected_return = self.resolve(&m.return_type);
            for s in &m.body.clone() { self.check_stmt(s, &mut env); }
        }
        self.current_class = None;
        self.type_params   = HashSet::new();
    }

    // ── Instructions ──────────────────────────────────────────────────────────

    fn check_stmt(&mut self, stmt: &Stmt, env: &mut TypeEnv) {
        match stmt {

            Stmt::VarDecl { ty, name, init } => {
                let resolved = self.resolve(ty);

                if let Some(init_expr) = init {
                    // Cas spécial : lambda littérale assignée à un type fn annoté
                    if let (Expr::Lambda { params, body }, Type::FnType(ptys, rty))
                        = (init_expr, &resolved)
                    {
                        let ptys = ptys.iter().map(|t| self.resolve(t)).collect::<Vec<_>>();
                        let rty  = self.resolve(rty);
                        self.check_lambda_typed(params, body, &ptys, &rty, env);
                    } else {
                        if let Some(et) = self.infer(init_expr, env) {
                            if !self.is_compatible(&et, &resolved) {
                                self.err(format!(
                                    "Déclaration '{}' : {} incompatible avec {}",
                                    name, et, resolved
                                ));
                            }
                        }
                    }
                }
                env.declare(name.clone(), resolved);
            }

            Stmt::Assign { target, value } => {
                let vt = self.infer(value, env);
                let tt = env.get(target).cloned()
                    .or_else(|| self.field_of_current_class(target))
                    .map(|t| self.resolve(&t));
                if let (Some(vt), Some(tt)) = (vt, tt) {
                    if !self.is_compatible(&vt, &tt) {
                        self.err(format!("Affectation '{}' : {} ≠ {}", target, vt, tt));
                    }
                }
            }

            Stmt::FieldAssign { object, field, value } => {
                let vt = self.infer(value, env);
                let ot = if object == "this" {
                    self.current_class.as_ref().map(|c| Type::UserDefined(c.clone()))
                } else {
                    env.get(object).cloned().or_else(|| self.field_of_current_class(object))
                };
                if let Some(ot) = ot {
                    let ot = self.resolve(&ot);
                    match self.find_field_type(&ot, field) {
                        Some(ft) => {
                            if let Some(vt) = vt {
                                if !self.is_compatible(&vt, &ft) {
                                    self.err(format!("{}.{} : {} ≠ {}", object, field, vt, ft));
                                }
                            }
                        }
                        None => warn!("Champ inconnu '{}.{}'", object, field),
                    }
                }
            }

            Stmt::Print(args) => { for a in args { self.infer(a, env); } }

            Stmt::Return(expr) => {
                let ret = self.expected_return.clone();
                match expr {
                    Some(e) => {
                        if let Some(ty) = self.infer(e, env) {
                            // Type::Fn = sentinelle "tout type accepté" (intérieur de lambda)
                            if !matches!(ret, Type::Fn) && !self.is_compatible(&ty, &ret) {
                                self.err(format!("return {} mais attendu {}", ty, ret));
                            }
                        }
                    }
                    None => {
                        if !matches!(ret, Type::Fn) && ret != Type::Void {
                            self.err(format!("return vide mais attendu {}", ret));
                        }
                    }
                }
            }

            Stmt::ExprStmt(e) => { self.infer(e, env); }

            Stmt::If { condition, then_body, else_body } => {
                if let Some(ct) = self.infer(condition, env) {
                    if ct != Type::Bool {
                        self.err(format!("Condition if doit être bool, trouvé {}", ct));
                    }
                }
                env.push();
                for s in then_body { self.check_stmt(s, env); }
                env.pop();
                if let Some(eb) = else_body {
                    env.push();
                    for s in eb { self.check_stmt(s, env); }
                    env.pop();
                }
            }

            Stmt::While { condition, body } | Stmt::DoWhile { body, condition } => {
                if let Some(ct) = self.infer(condition, env) {
                    if ct != Type::Bool {
                        self.err(format!("Condition while doit être bool, trouvé {}", ct));
                    }
                }
                env.push();
                for s in body { self.check_stmt(s, env); }
                env.pop();
            }

            Stmt::For { init, condition, update, body } => {
                env.push();
                if let Some(s) = init { self.check_stmt(s, env); }
                if let Some(e) = condition {
                    if let Some(ct) = self.infer(e, env) {
                        if ct != Type::Bool {
                            self.err(format!("Condition for doit être bool, trouvé {}", ct));
                        }
                    }
                }
                if let Some(s) = update { self.check_stmt(s, env); }
                env.push();
                for s in body { self.check_stmt(s, env); }
                env.pop();
                env.pop();
            }

            Stmt::Break | Stmt::Continue => {}

            Stmt::Match { expr, arms } => {
                let st = self.infer(expr, env);
                let enum_name = st.as_ref().and_then(|t| match t {
                    Type::UserDefined(n) => Some(n.clone()),
                    _ => None,
                });
                for arm in arms {
                    env.push();
                    if let (Pattern::Variant { name: vname, bindings }, Some(en))
                        = (&arm.pattern, &enum_name)
                    {
                        if let Some(ed) = self.enums.get(en).cloned() {
                            match ed.variants.iter().find(|v| &v.name == vname) {
                                None => self.err(format!("Variante '{}' inconnue dans '{}'", vname, en)),
                                Some(v) => {
                                    if !bindings.is_empty() && bindings.len() != v.fields.len() {
                                        self.err(format!(
                                            "Variante '{}' : {} binding(s) mais {} champ(s)",
                                            vname, bindings.len(), v.fields.len()
                                        ));
                                    }
                                    for (b, f) in bindings.iter().zip(v.fields.iter()) {
                                        env.declare(b.clone(), self.resolve(&f.ty));
                                    }
                                }
                            }
                        }
                    }
                    for s in &arm.body { self.check_stmt(s, env); }
                    env.pop();
                }
            }
        }
    }

    // ── Vérification d'une lambda avec types annotés ──────────────────────────
    //
    //  Appelée quand on a : `fn(int, int) -> int f = (a, b) => a + b;`
    //  On injecte les types des paramètres déclarés dans le scope de la lambda.

    fn check_lambda_typed(
        &mut self,
        params:    &[String],
        body:      &LambdaBody,
        param_tys: &[Type],
        ret_ty:    &Type,
        outer_env: &TypeEnv,
    ) {
        if params.len() != param_tys.len() {
            self.err(format!(
                "Lambda : {} paramètre(s) mais type déclare {}",
                params.len(), param_tys.len()
            ));
            return;
        }
        let mut lenv = TypeEnv::new();
        lenv.push();
        // Capture les variables visibles de l'environnement extérieur
        for (k, v) in outer_env.snapshot() { lenv.declare(k, v); }
        // Paramètres typés
        for (p, t) in params.iter().zip(param_tys.iter()) {
            lenv.declare(p.clone(), t.clone());
        }
        let saved = self.expected_return.clone();
        self.expected_return = ret_ty.clone();

        match body {
            LambdaBody::Expr(e) => {
                if let Some(actual) = self.infer(e, &lenv) {
                    if !self.is_compatible(&actual, ret_ty) {
                        self.err(format!(
                            "Corps de lambda : retourne {} mais {} attendu",
                            actual, ret_ty
                        ));
                    }
                }
            }
            LambdaBody::Block(stmts) => {
                let mut le = lenv;
                for s in stmts { self.check_stmt(s, &mut le); }
            }
        }
        self.expected_return = saved;
    }

    // ── Inférence de type ─────────────────────────────────────────────────────

    fn infer_expr(&mut self, expr: &Expr, env: &TypeEnv) -> Result<Type, TypeError> {
        match expr {
            Expr::IntLit(_)    => Ok(Type::Int),
            Expr::FloatLit(_)  => Ok(Type::Float),
            Expr::BoolLit(_)   => Ok(Type::Bool),
            Expr::StringLit(_) => Ok(Type::Str),

            Expr::Ident(name) => {
                if name == "this" {
                    return Ok(self.current_class.as_ref()
                        .map(|c| Type::UserDefined(c.clone()))
                        .or_else(|| self.current_enum.as_ref().map(|e| Type::UserDefined(e.clone())))
                        .unwrap_or(Type::Void));
                }
                if self.type_params.contains(name.as_str()) {
                    return Ok(Type::UserDefined(name.clone()));
                }
                let ty = env.get(name).cloned()
                    .or_else(|| self.field_of_current_class(name))
                    .or_else(|| self.field_of_current_enum(name))
                    .ok_or_else(|| TypeError(format!("Variable inconnue '{}'", name)))?;
                Ok(self.resolve(&ty))
            }

            Expr::UnaryOp { op, expr } => {
                let t = self.infer_expr(expr, env)?;
                match op {
                    UnaryOp::Neg => {
                        // Type::Fn = param non annoté → on skip
                        if matches!(t, Type::Fn) { return Ok(Type::Fn); }
                        if self.is_numeric(&t) { Ok(t) }
                        else { type_err!("- non applicable à {}", t) }
                    }
                    UnaryOp::Not => {
                        if matches!(t, Type::Fn) { return Ok(Type::Bool); }
                        if t == Type::Bool { Ok(Type::Bool) }
                        else { type_err!("! non applicable à {}", t) }
                    }
                }
            }

            Expr::BinOp { left, op, right } => {
                let lt = self.infer_expr(left, env)?;
                let rt = self.infer_expr(right, env)?;
                self.check_binop(&lt, op, &rt)
            }

            Expr::FieldAccess { object, field } => {
                let ot = self.infer_expr(object, env)?;
                let ot = self.resolve(&ot);
                self.find_field_type(&ot, field)
                    .ok_or_else(|| TypeError(format!("Champ inconnu '{}.{}'", ot, field)))
            }

            Expr::MethodCall { object, method, args } => {
                let ot = self.infer_expr(object, env)?;
                let ot = self.resolve(&ot);
                let (ptys, rty, subst) = self.resolve_method(&ot, method)?;
                if args.len() != ptys.len() {
                    return type_err!("{}() : {} arg(s) attendus, {} fournis",
                        method, ptys.len(), args.len());
                }
                for (arg, pt) in args.iter().zip(ptys.iter()) {
                    if let Ok(at) = self.infer_expr(arg, env) {
                        let expected = substitute(pt, &subst);
                        if !self.is_compatible(&at, &expected) {
                            self.err(format!("Arg de {}() : {} ≠ {}", method, at, expected));
                        }
                    }
                }
                Ok(self.resolve(&substitute(&rty, &subst)))
            }

            Expr::FunctionCall { name, args } => {
                if name == "print" {
                    for a in args { self.infer(a, env); }
                    return Ok(Type::Void);
                }

                // Lambda dans une variable locale (typée ou non)
                if let Some(ty) = env.get(name).cloned().map(|t| self.resolve(&t)) {
                    match &ty {
                        Type::FnType(ptys, rty) => {
                            let ptys = ptys.clone(); let rty = *rty.clone();
                            if args.len() != ptys.len() {
                                return type_err!("{}() : {} arg(s) attendus", name, ptys.len());
                            }
                            for (arg, pt) in args.iter().zip(ptys.iter()) {
                                if let Some(at) = self.infer(arg, env) {
                                    if !self.is_compatible(&at, pt) {
                                        self.err(format!("Arg de {}() : {} ≠ {}", name, at, pt));
                                    }
                                }
                            }
                            return Ok(rty);
                        }
                        Type::Fn => {
                            // Lambda non annotée : on skip la vérification des types
                            for a in args { self.infer(a, env); }
                            return Ok(Type::Fn);
                        }
                        _ => {}
                    }
                }

                // Méthode de la classe courante
                if let Some(cn) = self.current_class.clone() {
                    if let Some(m) = self.find_method_def(&cn, name).cloned() {
                        if args.len() != m.params.len() {
                            return type_err!("{}() : {} arg(s) attendus", name, m.params.len());
                        }
                        for (arg, p) in args.iter().zip(m.params.iter()) {
                            if let Ok(at) = self.infer_expr(arg, env) {
                                if !self.is_compatible(&at, &p.ty) {
                                    self.err(format!("Arg de {}() : {} ≠ {}", name, at, p.ty));
                                }
                            }
                        }
                        return Ok(self.resolve(&m.return_type));
                    }
                }
                if let Some(en) = self.current_enum.clone() {
                    if let Some(ed) = self.enums.get(&en).cloned() {
                        if let Some(m) = ed.methods.iter().find(|m| m.name == *name).cloned() {
                            return Ok(self.resolve(&m.return_type));
                        }
                    }
                }
                type_err!("Fonction inconnue '{}'", name)
            }

            Expr::New { class_name, type_args, args } => {
                let class = self.classes.get(class_name)
                    .ok_or_else(|| TypeError(format!("Classe inconnue '{}'", class_name)))?.clone();
                let subst: Vec<_> = class.type_params.iter()
                    .zip(type_args.iter()).map(|(p, t)| (p.clone(), t.clone())).collect();
                if !class.constructors.is_empty() {
                    let ctor = class.constructors.iter()
                        .find(|c| c.params.len() == args.len())
                        .ok_or_else(|| TypeError(format!(
                            "Pas de constructeur à {} arg(s) pour '{}'", args.len(), class_name
                        )))?.clone();
                    for (arg, p) in args.iter().zip(ctor.params.iter()) {
                        if let Ok(at) = self.infer_expr(arg, env) {
                            let expected = substitute(&p.ty, &subst);
                            if !self.is_compatible(&at, &expected) {
                                self.err(format!("new {}() : {} ≠ {}", class_name, at, expected));
                            }
                        }
                    }
                } else if !args.is_empty() {
                    self.err(format!("'{}' n'a pas de constructeur", class_name));
                }
                if type_args.is_empty() { Ok(Type::UserDefined(class_name.clone())) }
                else { Ok(Type::Generic(class_name.clone(), type_args.clone())) }
            }

            Expr::EnumConstructor { enum_name, variant, args } => {
                let ed = self.enums.get(enum_name)
                    .ok_or_else(|| TypeError(format!("Enum inconnu '{}'", enum_name)))?.clone();
                let vd = ed.variants.iter().find(|v| &v.name == variant)
                    .ok_or_else(|| TypeError(format!(
                        "Variante '{}' inconnue dans '{}'", variant, enum_name)))?.clone();
                if args.len() != vd.fields.len() {
                    return type_err!("'{}::{}' : {} champ(s), {} fourni(s)",
                        enum_name, variant, vd.fields.len(), args.len());
                }
                for (arg, f) in args.iter().zip(vd.fields.iter()) {
                    if let Ok(at) = self.infer_expr(arg, env) {
                        if !self.is_compatible(&at, &f.ty) {
                            self.err(format!("Champ '{}' de '{}::{}' : {} ≠ {}",
                                f.name, enum_name, variant, at, f.ty));
                        }
                    }
                }
                Ok(Type::UserDefined(enum_name.clone()))
            }

            // ── Lambda non annotée : sentinelle Type::Fn ──────────────────────
            Expr::Lambda { params, body } => {
                let mut lenv = TypeEnv::new(); lenv.push();
                for (k, v) in env.snapshot() { lenv.declare(k, v); }
                for p in params { lenv.declare(p.clone(), Type::Fn); }
                let saved = self.expected_return.clone();
                // Type::Fn = sentinelle "tout type de retour accepté"
                self.expected_return = Type::Fn;
                match body {
                    LambdaBody::Expr(e)      => { self.infer(e, &lenv); }
                    LambdaBody::Block(stmts) => {
                        let mut le = lenv;
                        for s in stmts { self.check_stmt(s, &mut le); }
                    }
                }
                self.expected_return = saved;
                Ok(Type::Fn)
            }

            // ── Appel de lambda ───────────────────────────────────────────────
            Expr::LambdaCall { callee, args } => {
                let ct = self.infer_expr(callee, env)?;
                let ct = self.resolve(&ct);
                match ct {
                    Type::FnType(ptys, rty) => {
                        if args.len() != ptys.len() {
                            return type_err!("Lambda : {} arg(s) attendus, {} fournis",
                                ptys.len(), args.len());
                        }
                        for (arg, pt) in args.iter().zip(ptys.iter()) {
                            if let Some(at) = self.infer(arg, env) {
                                if !self.is_compatible(&at, pt) {
                                    self.err(format!("Arg lambda : {} ≠ {}", at, pt));
                                }
                            }
                        }
                        Ok(*rty)
                    }
                    Type::Fn => {
                        for a in args { self.infer(a, env); }
                        Ok(Type::Fn) // type de retour inconnu — compatible avec tout
                    }
                    _ => type_err!("LambdaCall sur non-lambda ({})", ct),
                }
            }
        }
    }

    // ── Compatibilité de types ────────────────────────────────────────────────

    pub fn is_compatible(&self, actual: &Type, expected: &Type) -> bool {
        // Résolution des alias pour la comparaison
        let actual   = self.resolve(actual);
        let expected = self.resolve(expected);

        if let Type::UserDefined(n) = &actual   { if self.type_params.contains(n) { return true; } }
        if let Type::UserDefined(n) = &expected { if self.type_params.contains(n) { return true; } }
        if actual == expected { return true; }
        if matches!(actual, Type::Void) { return true; }

        // Type::Fn (non annoté) = compatible avec TOUT
        if matches!(actual, Type::Fn) || matches!(expected, Type::Fn) { return true; }

        // Promotions numériques
        if matches!(expected, Type::Double) && self.is_numeric(&actual) { return true; }
        if matches!(expected, Type::Float)  && matches!(actual, Type::Int) { return true; }

        // Sous-typage de classes
        match (&actual, &expected) {
            (Type::UserDefined(a), Type::UserDefined(e)) => self.is_subclass(a, e),
            (Type::Generic(an, _), Type::UserDefined(en)) => self.is_subclass(an, en),
            (Type::UserDefined(an), Type::Generic(en, _)) => self.is_subclass(an, en),
            (Type::Generic(an, aa), Type::Generic(en, ea)) =>
                an == en && aa.len() == ea.len()
                    && aa.iter().zip(ea.iter()).all(|(a, e)| self.is_compatible(a, e)),
            // FnType compatible si params et retour compatibles
            (Type::FnType(ap, ar), Type::FnType(ep, er)) =>
                ap.len() == ep.len()
                    && ap.iter().zip(ep.iter()).all(|(a, e)| self.is_compatible(a, e))
                    && self.is_compatible(ar, er),
            _ => false,
        }
    }

    fn is_subclass(&self, sub: &str, sup: &str) -> bool {
        if sub == sup { return true; }
        if let Some(c) = self.classes.get(sub) {
            if let Some(p) = &c.parent { return self.is_subclass(p, sup); }
        }
        false
    }

    // ── Opérateurs binaires ───────────────────────────────────────────────────

    fn check_binop(&mut self, lt: &Type, op: &BinOp, rt: &Type) -> Result<Type, TypeError> {
        // Type::Fn (param non annoté) → on skip la vérification, type résultat inconnu
        if matches!(lt, Type::Fn) || matches!(rt, Type::Fn) {
            return Ok(match op {
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le
                | BinOp::Gt | BinOp::Ge | BinOp::And | BinOp::Or => Type::Bool,
                _ => Type::Fn,
            });
        }
        match op {
            BinOp::Add => {
                if self.is_numeric(lt) && self.is_numeric(rt) { Ok(self.numeric_result(lt, rt)) }
                else if matches!((lt, rt), (Type::Str, Type::Str)) { Ok(Type::Str) }
                else { type_err!("+ non applicable à {} et {}", lt, rt) }
            }
            BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow => {
                if self.is_numeric(lt) && self.is_numeric(rt) { Ok(self.numeric_result(lt, rt)) }
                else { type_err!("Opérateur arithmétique non applicable à {} et {}", lt, rt) }
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                if self.is_numeric(lt) && self.is_numeric(rt) { Ok(Type::Bool) }
                else { type_err!("Comparaison non applicable à {} et {}", lt, rt) }
            }
            BinOp::Eq | BinOp::Ne => Ok(Type::Bool),
            BinOp::And | BinOp::Or => {
                if matches!((lt, rt), (Type::Bool, Type::Bool)) { Ok(Type::Bool) }
                else { type_err!("&& / || requièrent bool, trouvé {} et {}", lt, rt) }
            }
        }
    }

    fn is_numeric(&self, t: &Type) -> bool { matches!(t, Type::Int | Type::Float | Type::Double) }

    fn numeric_result(&self, lt: &Type, rt: &Type) -> Type {
        match (lt, rt) {
            (Type::Double, _) | (_, Type::Double) => Type::Double,
            (Type::Float,  _) | (_, Type::Float)  => Type::Float,
            _ => Type::Int,
        }
    }

    // ── Résolution de méthode ─────────────────────────────────────────────────

    fn resolve_method(&self, obj_ty: &Type, method: &str)
        -> Result<(Vec<Type>, Type, Vec<(String, Type)>), TypeError>
    {
        let (cn, type_args) = match obj_ty {
            Type::UserDefined(n)   => (n.clone(), vec![]),
            Type::Generic(n, args) => (n.clone(), args.clone()),
            _ => return type_err!("Appel '{}' sur type non-objet {}", method, obj_ty),
        };

        if let Some(m) = self.find_method_def(&cn, method) {
            let m = m.clone();
            let subst: Vec<_> = self.classes.get(&cn).map(|c| {
                c.type_params.iter().zip(type_args.iter())
                    .map(|(p, t)| (p.clone(), t.clone())).collect()
            }).unwrap_or_default();
            return Ok((m.params.iter().map(|p| p.ty.clone()).collect(), m.return_type, subst));
        }
        if let Some(ed) = self.enums.get(&cn) {
            if let Some(m) = ed.methods.iter().find(|m| m.name == method) {
                return Ok((m.params.iter().map(|p| p.ty.clone()).collect(), m.return_type.clone(), vec![]));
            }
        }
        type_err!("Méthode '{}' inconnue dans '{}'", method, cn)
    }

    // ── Lookup ────────────────────────────────────────────────────────────────

    fn find_method_def<'a>(&'a self, cn: &str, mn: &str) -> Option<&'a Method> {
        let c = self.classes.get(cn)?;
        if let Some(m) = c.methods.iter().find(|m| m.name == mn) { return Some(m); }
        if let Some(p) = &c.parent { return self.find_method_def(p, mn); }
        None
    }

    fn find_field_type(&self, obj_ty: &Type, field: &str) -> Option<Type> {
        let cn = match obj_ty {
            Type::UserDefined(n) | Type::Generic(n, _) => n.clone(),
            _ => return None,
        };
        self.find_field_in(&cn, field)
    }

    fn find_field_in(&self, cn: &str, field: &str) -> Option<Type> {
        let c = self.classes.get(cn)?;
        if let Some(f) = c.fields.iter().find(|f| f.name == field) {
            return Some(self.resolve(&f.ty));
        }
        if let Some(p) = &c.parent { return self.find_field_in(p, field); }
        None
    }

    fn field_of_current_class(&self, field: &str) -> Option<Type> {
        self.current_class.as_ref().and_then(|cn| self.find_field_in(cn, field))
    }

    fn field_of_current_enum(&self, field: &str) -> Option<Type> {
        let en = self.current_enum.as_ref()?;
        let ed = self.enums.get(en)?;
        ed.variants.iter().flat_map(|v| &v.fields)
            .find(|p| p.name == field)
            .map(|p| self.resolve(&p.ty))
    }
}

// ── API de test ───────────────────────────────────────────────────────────────

pub fn check_source(src: &str) -> Result<(), Vec<String>> {
    use chumsky::Parser;
    let program = crate::parser::program_parser()
        .parse(src)
        .map_err(|e| e.iter().map(|x| x.to_string()).collect::<Vec<_>>())?;
    let errors = TypeChecker::new(&program).check(&program);
    if errors.is_empty() { Ok(()) }
    else { Err(errors.iter().map(|e| e.0.clone()).collect()) }
}
