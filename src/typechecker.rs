// ─────────────────────────────────────────────────────────────────────────────
//  Typechecker – vérifie les types avant l'interprétation
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::{HashMap, HashSet};
use log::{debug, warn};

use crate::ast::*;

// ── Erreur de typage ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TypeError(pub String);

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypeError: {}", self.0)
    }
}

macro_rules! type_err {
    ($($arg:tt)*) => { Err(TypeError(format!($($arg)*))) };
}

// ── Environnement de types ────────────────────────────────────────────────────

struct TypeEnv {
    scopes: Vec<HashMap<String, Type>>,
}

impl TypeEnv {
    fn new() -> Self { Self { scopes: vec![HashMap::new()] } }
    fn push(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop(&mut self) { if self.scopes.len() > 1 { self.scopes.pop(); } }
    fn declare(&mut self, name: String, ty: Type) {
        self.scopes.last_mut().unwrap().insert(name, ty);
    }
    fn get(&self, name: &str) -> Option<&Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) { return Some(t); }
        }
        None
    }
}

// ── Substitution de paramètres génériques ─────────────────────────────────────

fn substitute(ty: &Type, subst: &[(String, Type)]) -> Type {
    match ty {
        Type::UserDefined(n) => subst.iter()
            .find(|(k, _)| k == n)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| ty.clone()),
        Type::Array(inner) => Type::Array(Box::new(substitute(inner, subst))),
        Type::Generic(n, args) => {
            Type::Generic(n.clone(), args.iter().map(|a| substitute(a, subst)).collect())
        }
        _ => ty.clone(),
    }
}

// ── TypeChecker ───────────────────────────────────────────────────────────────

pub struct TypeChecker {
    classes:         HashMap<String, ClassDef>,
    interfaces:      HashMap<String, InterfaceDef>,
    enums:           HashMap<String, EnumDef>,
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

    fn err(&mut self, msg: String) { self.errors.push(TypeError(msg)); }

    /// Infère et enregistre l'erreur si besoin (évite que `if let Ok` l'ignore)
    fn infer(&mut self, expr: &Expr, env: &TypeEnv) -> Option<Type> {
        match self.infer_expr(expr, env) {
            Ok(t)  => Some(t),
            Err(e) => { self.errors.push(e); None }
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

    // ── Hiérarchie de classes ─────────────────────────────────────────────────

    fn check_class_hierarchy(&mut self) {
        for name in &self.classes.keys().cloned().collect::<Vec<_>>() {
            let class = self.classes[name].clone();
            if let Some(parent) = &class.parent {
                if !self.classes.contains_key(parent) {
                    self.err(format!("Classe '{}' extends '{}' inconnu", name, parent));
                }
            }
            for iface in &class.implements {
                if !self.interfaces.contains_key(iface) {
                    self.err(format!("Classe '{}' implements '{}' inconnu", name, iface));
                }
            }
            if self.has_inheritance_cycle(name) {
                self.err(format!("Cycle d'héritage détecté pour '{}'", name));
            }
        }
    }

    fn has_inheritance_cycle(&self, start: &str) -> bool {
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

    // ── Interfaces ────────────────────────────────────────────────────────────

    fn check_interface_impls(&mut self) {
        for class_name in &self.classes.keys().cloned().collect::<Vec<_>>() {
            let class = self.classes[class_name].clone();
            for iface_name in &class.implements.clone() {
                if let Some(iface) = self.interfaces.get(iface_name).cloned() {
                    for sig in &iface.methods {
                        if self.find_method_def(class_name, &sig.name).is_none() {
                            self.err(format!(
                                "Classe '{}' n'implémente pas '{}.{}()'",
                                class_name, iface_name, sig.name
                            ));
                        }
                    }
                }
            }
        }
    }

    // ── Enums ─────────────────────────────────────────────────────────────────

    fn check_enums(&mut self, program: &Program) {
        for enum_def in &program.enums.clone() {
            self.current_enum = Some(enum_def.name.clone());
            debug!("  check enum '{}'", enum_def.name);

            // Noms de variantes uniques ?
            let mut seen = HashSet::new();
            for v in &enum_def.variants {
                if !seen.insert(v.name.clone()) {
                    self.err(format!("Enum '{}' : variante '{}' dupliquée", enum_def.name, v.name));
                }
            }

            // Méthodes de l'enum
            for method in &enum_def.methods.clone() {
                let mut env = TypeEnv::new();
                env.push();
                for p in &method.params { env.declare(p.name.clone(), p.ty.clone()); }
                self.expected_return = method.return_type.clone();
                for stmt in &method.body.clone() { self.check_stmt(stmt, &mut env); }
            }

            self.current_enum = None;
        }
    }

    // ── Classe ────────────────────────────────────────────────────────────────

    fn check_class(&mut self, class: &ClassDef) {
        self.current_class = Some(class.name.clone());
        self.type_params   = class.type_params.iter().cloned().collect();
        debug!("  check class '{}'", class.name);

        for ctor in &class.constructors.clone() {
            let mut env = TypeEnv::new();
            env.push();
            for p in &ctor.params { env.declare(p.name.clone(), p.ty.clone()); }
            self.expected_return = Type::Void;
            for stmt in &ctor.body.clone() { self.check_stmt(stmt, &mut env); }
        }
        for method in &class.methods.clone() {
            let mut env = TypeEnv::new();
            env.push();
            for p in &method.params { env.declare(p.name.clone(), p.ty.clone()); }
            self.expected_return = method.return_type.clone();
            for stmt in &method.body.clone() { self.check_stmt(stmt, &mut env); }
        }

        self.current_class = None;
        self.type_params   = HashSet::new();
    }

    // ── Instructions ──────────────────────────────────────────────────────────

    fn check_stmt(&mut self, stmt: &Stmt, env: &mut TypeEnv) {
        match stmt {
            Stmt::VarDecl { ty, name, init } => {
                if let Some(expr) = init {
                    if let Some(expr_ty) = self.infer(expr, env) {
                        if !self.is_compatible(&expr_ty, ty) {
                            self.err(format!(
                                "Déclaration '{}' : type {} incompatible avec {}",
                                name, expr_ty, ty
                            ));
                        }
                    }
                }
                env.declare(name.clone(), ty.clone());
            }

            Stmt::Assign { target, value } => {
                let val_ty = self.infer(value, env);
                let target_ty = env.get(target).cloned()
                    .or_else(|| self.field_type_of_current_class(target))
                    .or_else(|| self.field_type_of_current_enum_variant(target));
                if let (Some(vt), Some(tt)) = (val_ty, target_ty) {
                    if !self.is_compatible(&vt, &tt) {
                        self.err(format!("Affectation '{}' : {} incompatible avec {}", target, vt, tt));
                    }
                }
            }

            Stmt::FieldAssign { object, field, value } => {
                let val_ty = self.infer(value, env);
                let obj_ty = if object == "this" {
                    self.current_class.as_ref().map(|c| Type::UserDefined(c.clone()))
                        .or_else(|| self.current_enum.as_ref().map(|e| Type::UserDefined(e.clone())))
                } else {
                    env.get(object).cloned().or_else(|| self.field_type_of_current_class(object))
                };
                if let Some(ot) = obj_ty {
                    match self.find_field_type_of(&ot, field) {
                        Some(ft) => {
                            if let Some(vt) = val_ty {
                                if !self.is_compatible(&vt, &ft) {
                                    self.err(format!(
                                        "Affectation {}.{} : {} incompatible avec {}",
                                        object, field, vt, ft
                                    ));
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
                if let Some(e) = expr {
                    if let Some(ty) = self.infer(e, env) {
                        // Type::Fn = sentinelle "tout type accepté" (corps de lambda)
                        if !matches!(ret, Type::Fn) && !self.is_compatible(&ty, &ret) {
                            self.err(format!("return {} mais attendu {}", ty, ret));
                        }
                    }
                } else if !matches!(ret, Type::Fn) && ret != Type::Void {
                    self.err(format!("return vide mais type attendu {}", ret));
                }
            }

            Stmt::ExprStmt(e) => { self.infer(e, env); }

            Stmt::If { condition, then_body, else_body } => {
                match self.infer(condition, env) {
                    Some(ct) if ct != Type::Bool =>
                        self.err(format!("Condition if doit être bool, trouvé {}", ct)),
                    _ => {}
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
                match self.infer(condition, env) {
                    Some(ct) if ct != Type::Bool =>
                        self.err(format!("Condition while doit être bool, trouvé {}", ct)),
                    _ => {}
                }
                env.push();
                for s in body { self.check_stmt(s, env); }
                env.pop();
            }

            Stmt::For { init, condition, update, body } => {
                env.push();
                if let Some(s) = init { self.check_stmt(s, env); }
                if let Some(e) = condition {
                    match self.infer(e, env) {
                        Some(ct) if ct != Type::Bool =>
                            self.err(format!("Condition for doit être bool, trouvé {}", ct)),
                        _ => {}
                    }
                }
                if let Some(s) = update { self.check_stmt(s, env); }
                env.push();
                for s in body { self.check_stmt(s, env); }
                env.pop();
                env.pop();
            }

            Stmt::Break | Stmt::Continue => {}

            // ── match ─────────────────────────────────────────────────────────
            Stmt::Match { expr, arms } => {
                let scrutinee_ty = self.infer(expr, env);

                // Trouve l'enum scrutiné
                let enum_name = scrutinee_ty.as_ref().and_then(|t| match t {
                    Type::UserDefined(n) => Some(n.clone()),
                    _ => None,
                });

                for arm in arms {
                    env.push();

                    match &arm.pattern {
                        Pattern::Wildcard => {}
                        Pattern::Variant { name: variant_name, bindings } => {
                            // Vérifie que la variante existe dans l'enum
                            if let Some(ref en) = enum_name {
                                if let Some(enum_def) = self.enums.get(en).cloned() {
                                    match enum_def.variants.iter().find(|v| &v.name == variant_name) {
                                        None => self.err(format!(
                                            "Variante '{}' inconnue dans l'enum '{}'",
                                            variant_name, en
                                        )),
                                        Some(variant) => {
                                            // Vérifie le nombre de bindings
                                            if bindings.len() != variant.fields.len()
                                                && !bindings.is_empty()
                                            {
                                                self.err(format!(
                                                    "Variante '{}' : {} champ(s) attendu(s), {} binding(s) fourni(s)",
                                                    variant_name, variant.fields.len(), bindings.len()
                                                ));
                                            }
                                            // Déclare les bindings avec le type du champ correspondant
                                            for (binding, field) in bindings.iter().zip(variant.fields.iter()) {
                                                env.declare(binding.clone(), field.ty.clone());
                                            }
                                        }
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
                        .or_else(|| self.current_enum.as_ref()
                            .map(|e| Type::UserDefined(e.clone())))
                        .unwrap_or(Type::Void));
                }
                if self.type_params.contains(name.as_str()) {
                    return Ok(Type::UserDefined(name.clone()));
                }
                env.get(name).cloned()
                    .or_else(|| self.field_type_of_current_class(name))
                    .or_else(|| self.field_type_of_current_enum_variant(name))
                    .ok_or_else(|| TypeError(format!("Variable inconnue '{}'", name)))
            }

            Expr::UnaryOp { op, expr } => {
                let t = self.infer_expr(expr, env)?;
                match op {
                    UnaryOp::Neg => {
                        if self.is_numeric(&t) { Ok(t) }
                        else { type_err!("Opérateur - non applicable à {}", t) }
                    }
                    UnaryOp::Not => {
                        if t == Type::Bool { Ok(Type::Bool) }
                        else { type_err!("Opérateur ! non applicable à {}", t) }
                    }
                }
            }

            Expr::BinOp { left, op, right } => {
                let lt = self.infer_expr(left, env)?;
                let rt = self.infer_expr(right, env)?;
                self.check_binop(&lt, op, &rt)
            }

            Expr::FieldAccess { object, field } => {
                let obj_ty = self.infer_expr(object, env)?;
                self.find_field_type_of(&obj_ty, field)
                    .ok_or_else(|| TypeError(format!("Champ inconnu '{}.{}'", obj_ty, field)))
            }

            Expr::MethodCall { object, method, args } => {
                let obj_ty = self.infer_expr(object, env)?;
                let (param_types, return_type, subst) = self.resolve_method(&obj_ty, method)?;
                if args.len() != param_types.len() {
                    return type_err!("{}() : {} arg(s) attendus, {} fournis",
                        method, param_types.len(), args.len());
                }
                for (arg, pt) in args.iter().zip(param_types.iter()) {
                    let at = self.infer_expr(arg, env)?;
                    let expected = substitute(pt, &subst);
                    if !self.is_compatible(&at, &expected) {
                        self.err(format!("Argument de {}() : {} incompatible avec {}", method, at, expected));
                    }
                }
                Ok(substitute(&return_type, &subst))
            }

            Expr::FunctionCall { name, args } => {
                if name == "print" {
                    for a in args { self.infer(a, env); }
                    return Ok(Type::Void);
                }

                // Appel d'une lambda stockée dans une variable locale
                if let Some(ty) = env.get(name) {
                    if self.is_lambda_type(ty) {
                        for a in args { self.infer(a, env); }
                        return Ok(Type::Void); // type de retour non inféré
                    }
                }

                if let Some(class_name) = self.current_class.clone() {
                    if let Some(m) = self.find_method_def(&class_name, name).cloned() {
                        if args.len() != m.params.len() {
                            return type_err!("{}() : {} arg(s) attendus", name, m.params.len());
                        }
                        for (arg, p) in args.iter().zip(m.params.iter()) {
                            let at = self.infer_expr(arg, env)?;
                            if !self.is_compatible(&at, &p.ty) {
                                self.err(format!("Arg de {}() : {} incompatible avec {}", name, at, p.ty));
                            }
                        }
                        return Ok(m.return_type.clone());
                    }
                }
                if let Some(enum_name) = self.current_enum.clone() {
                    if let Some(ed) = self.enums.get(&enum_name).cloned() {
                        if let Some(m) = ed.methods.iter().find(|m| m.name == *name).cloned() {
                            return Ok(m.return_type.clone());
                        }
                    }
                }
                type_err!("Fonction inconnue '{}'", name)
            }

            // ── Lambda ────────────────────────────────────────────────────────
            Expr::Lambda { params, body } => {
                let mut lenv = TypeEnv::new();
                lenv.push();
                for p in params { lenv.declare(p.clone(), Type::Fn); }
                let saved = self.expected_return.clone();
                // Type::Fn comme sentinelle = "tout type de retour accepté"
                // (évite les faux positifs "return X mais attendu void")
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

            // ── Appel d'une lambda ────────────────────────────────────────────
            Expr::LambdaCall { callee, args } => {
                let callee_ty = self.infer_expr(callee, env)?;
                if !matches!(callee_ty, Type::Fn) {
                    self.err(format!("Appel sur non-lambda (type : {})", callee_ty));
                }
                for a in args { self.infer(a, env); }
                // Type de retour non connu statiquement : on retourne Void
                Ok(Type::Void)
            }

            Expr::New { class_name, type_args, args } => {
                let class = self.classes.get(class_name)
                    .ok_or_else(|| TypeError(format!("Classe inconnue '{}'", class_name)))?.clone();
                let subst: Vec<(String, Type)> = class.type_params.iter()
                    .zip(type_args.iter()).map(|(p, t)| (p.clone(), t.clone())).collect();
                if !class.constructors.is_empty() {
                    let ctor = class.constructors.iter()
                        .find(|c| c.params.len() == args.len())
                        .ok_or_else(|| TypeError(format!(
                            "Pas de constructeur à {} arg(s) pour '{}'", args.len(), class_name
                        )))?.clone();
                    for (arg, p) in args.iter().zip(ctor.params.iter()) {
                        let at = self.infer_expr(arg, env)?;
                        let expected = substitute(&p.ty, &subst);
                        if !self.is_compatible(&at, &expected) {
                            self.err(format!("new {}() : arg {} incompatible avec {}", class_name, at, expected));
                        }
                    }
                } else if !args.is_empty() {
                    self.err(format!("'{}' n'a pas de constructeur mais reçoit {} arg(s)", class_name, args.len()));
                }
                if type_args.is_empty() { Ok(Type::UserDefined(class_name.clone())) }
                else { Ok(Type::Generic(class_name.clone(), type_args.clone())) }
            }

            // ── Constructeur d'enum : EnumName::Variant(args) ────────────────
            Expr::EnumConstructor { enum_name, variant, args } => {
                let enum_def = self.enums.get(enum_name)
                    .ok_or_else(|| TypeError(format!("Enum inconnu '{}'", enum_name)))?.clone();
                let var_def = enum_def.variants.iter()
                    .find(|v| &v.name == variant)
                    .ok_or_else(|| TypeError(format!(
                        "Variante '{}' inconnue dans l'enum '{}'", variant, enum_name
                    )))?.clone();
                if args.len() != var_def.fields.len() {
                    return type_err!(
                        "Variante '{}::{}' : {} champ(s) attendu(s), {} fourni(s)",
                        enum_name, variant, var_def.fields.len(), args.len()
                    );
                }
                for (arg, field) in args.iter().zip(var_def.fields.iter()) {
                    let at = self.infer_expr(arg, env)?;
                    if !self.is_compatible(&at, &field.ty) {
                        self.err(format!(
                            "Champ '{}' de '{}::{}' : {} incompatible avec {}",
                            field.name, enum_name, variant, at, field.ty
                        ));
                    }
                }
                Ok(Type::UserDefined(enum_name.clone()))
            }
        }
    }

    // ── Compatibilité de types ────────────────────────────────────────────────

    pub fn is_compatible(&self, actual: &Type, expected: &Type) -> bool {
        if let Type::UserDefined(n) = actual   { if self.type_params.contains(n) { return true; } }
        if let Type::UserDefined(n) = expected { if self.type_params.contains(n) { return true; } }
        if actual == expected { return true; }
        if matches!(actual, Type::Void) { return true; }
        // Type::Fn = paramètre de lambda de type inconnu → compatible avec tout
        if matches!(actual, Type::Fn) || matches!(expected, Type::Fn) { return true; }
        if matches!(expected, Type::Double) && self.is_numeric(actual) { return true; }
        if matches!(expected, Type::Float) && matches!(actual, Type::Int) { return true; }
        match (actual, expected) {
            (Type::UserDefined(a), Type::UserDefined(e)) => self.is_subclass(a, e),
            (Type::Generic(an, _), Type::UserDefined(en)) => self.is_subclass(an, en),
            (Type::UserDefined(an), Type::Generic(en, _)) => self.is_subclass(an, en),
            (Type::Generic(an, aa), Type::Generic(en, ea)) =>
                an == en && aa.len() == ea.len()
                    && aa.iter().zip(ea.iter()).all(|(a, e)| self.is_compatible(a, e)),
            _ => false,
        }
    }

    fn is_subclass(&self, sub: &str, sup: &str) -> bool {
        if sub == sup { return true; }
        if let Some(class) = self.classes.get(sub) {
            if let Some(parent) = &class.parent { return self.is_subclass(parent, sup); }
        }
        // Un enum est compatible avec lui-même (déjà géré par sub == sup)
        false
    }

    // ── Opérateurs binaires ───────────────────────────────────────────────────

    fn check_binop(&mut self, lt: &Type, op: &BinOp, rt: &Type) -> Result<Type, TypeError> {
        match op {
            BinOp::Add => {
                if self.is_numeric(lt) && self.is_numeric(rt) { Ok(self.numeric_result(lt, rt)) }
                else if matches!((lt, rt), (Type::Str, Type::Str)) { Ok(Type::Str) }
                else { type_err!("Opérateur + non applicable à {} et {}", lt, rt) }
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
                else { type_err!("Opérateurs logiques requièrent bool, trouvé {} et {}", lt, rt) }
            }
        }
    }

    fn is_numeric(&self, t: &Type) -> bool { matches!(t, Type::Int | Type::Float | Type::Double) }

    fn is_lambda_type(&self, t: &Type) -> bool { matches!(t, Type::Fn) }

    fn numeric_result(&self, lt: &Type, rt: &Type) -> Type {
        match (lt, rt) {
            (Type::Double, _) | (_, Type::Double) => Type::Double,
            (Type::Float,  _) | (_, Type::Float)  => Type::Float,
            _                                      => Type::Int,
        }
    }

    // ── Résolution de méthode ─────────────────────────────────────────────────

    fn resolve_method(&self, obj_ty: &Type, method: &str)
        -> Result<(Vec<Type>, Type, Vec<(String, Type)>), TypeError>
    {
        let (class_name, type_args) = match obj_ty {
            Type::UserDefined(n)   => (n.clone(), vec![]),
            Type::Generic(n, args) => (n.clone(), args.clone()),
            _ => return type_err!("Appel de méthode '{}' sur type non-objet {}", method, obj_ty),
        };

        // Cherche dans les classes
        if let Some(m) = self.find_method_def(&class_name, method) {
            let m = m.clone();
            let class = self.classes.get(&class_name);
            let subst: Vec<(String, Type)> = class.map(|c| {
                c.type_params.iter().zip(type_args.iter())
                    .map(|(p, t)| (p.clone(), t.clone())).collect()
            }).unwrap_or_default();
            return Ok((m.params.iter().map(|p| p.ty.clone()).collect(), m.return_type.clone(), subst));
        }

        // Cherche dans les enums
        if let Some(ed) = self.enums.get(&class_name) {
            if let Some(m) = ed.methods.iter().find(|m| m.name == method) {
                return Ok((
                    m.params.iter().map(|p| p.ty.clone()).collect(),
                    m.return_type.clone(),
                    vec![],
                ));
            }
        }

        type_err!("Méthode '{}' inconnue dans '{}'", method, class_name)
    }

    // ── Lookup hiérarchique ───────────────────────────────────────────────────

    fn find_method_def<'a>(&'a self, class_name: &str, method_name: &str) -> Option<&'a Method> {
        let class = self.classes.get(class_name)?;
        if let Some(m) = class.methods.iter().find(|m| m.name == method_name) { return Some(m); }
        if let Some(parent) = &class.parent { return self.find_method_def(parent, method_name); }
        None
    }

    fn find_field_type_of(&self, obj_ty: &Type, field: &str) -> Option<Type> {
        let name = match obj_ty {
            Type::UserDefined(n) | Type::Generic(n, _) => n.clone(),
            _ => return None,
        };
        self.find_field_in_class(&name, field)
    }

    fn find_field_in_class(&self, class_name: &str, field: &str) -> Option<Type> {
        let class = self.classes.get(class_name)?;
        if let Some(f) = class.fields.iter().find(|f| f.name == field) { return Some(f.ty.clone()); }
        if let Some(parent) = &class.parent { return self.find_field_in_class(parent, field); }
        None
    }

    fn field_type_of_current_class(&self, field: &str) -> Option<Type> {
        self.current_class.as_ref()
            .and_then(|n| self.find_field_in_class(n, field))
    }

    /// Dans une méthode d'enum, `this` peut avoir les champs de n'importe quelle
    /// variante — on retourne le premier champ trouvé avec ce nom.
    fn field_type_of_current_enum_variant(&self, field: &str) -> Option<Type> {
        let enum_name = self.current_enum.as_ref()?;
        let ed = self.enums.get(enum_name)?;
        for v in &ed.variants {
            if let Some(p) = v.fields.iter().find(|p| p.name == field) {
                return Some(p.ty.clone());
            }
        }
        None
    }
}

// ── API de test ───────────────────────────────────────────────────────────────

pub fn check_source(src: &str) -> Result<(), Vec<String>> {
    use chumsky::Parser;
    let program = crate::parser::program_parser()
        .parse(src)
        .map_err(|errs| errs.iter().map(|e| e.to_string()).collect::<Vec<_>>())?;
    let errors = TypeChecker::new(&program).check(&program);
    if errors.is_empty() { Ok(()) }
    else { Err(errors.iter().map(|e| e.0.clone()).collect()) }
}
