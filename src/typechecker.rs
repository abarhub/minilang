// ─────────────────────────────────────────────────────────────────────────────
//  Vérificateur de types – passe avant l'interprétation
//
//  Ce qu'il vérifie :
//    • Tous les parents / interfaces référencés existent
//    • Absence de cycle d'héritage
//    • Implémentation complète des interfaces
//    • Existence des méthodes appelées (héritage inclus)
//    • Existence des champs accédés
//    • Compatibilité de types dans les affectations et les retours
//      – Primitifs : type exact requis
//      – Classes   : sous-type accepté
//      – Paramètres génériques (T, K…) : compatibles avec tout
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::{HashMap, HashSet};
use log::{debug, warn};

use crate::ast::*;

// ─────────────────────────────────────────────────────────────────────────────
//  Erreur de typage
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
//  Environnement de types (pile de scopes)
// ─────────────────────────────────────────────────────────────────────────────

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

    fn set(&mut self, name: &str, ty: Type) {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) { scope.insert(name.to_string(), ty); return; }
        }
        self.scopes.last_mut().unwrap().insert(name.to_string(), ty);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Utilitaire : substitution des paramètres génériques dans un type
//  ex. substituer(T→int, Array<T>) → Array<int>
// ─────────────────────────────────────────────────────────────────────────────

fn substitute(ty: &Type, subst: &[(String, Type)]) -> Type {
    match ty {
        Type::UserDefined(name) => {
            if let Some((_, replacement)) = subst.iter().find(|(n, _)| n == name) {
                replacement.clone()
            } else {
                ty.clone()
            }
        }
        Type::Array(inner) => Type::Array(Box::new(substitute(inner, subst))),
        Type::Generic(name, args) => {
            let new_args = args.iter().map(|a| substitute(a, subst)).collect();
            Type::Generic(name.clone(), new_args)
        }
        _ => ty.clone(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  TypeChecker
// ─────────────────────────────────────────────────────────────────────────────

pub struct TypeChecker {
    classes:        HashMap<String, ClassDef>,
    interfaces:     HashMap<String, InterfaceDef>,
    /// Paramètres de type de la classe en cours d'analyse
    type_params:    HashSet<String>,
    /// Classe en cours d'analyse (pour `this`)
    current_class:  Option<String>,
    /// Type de retour attendu dans la méthode en cours
    expected_return: Type,
    /// Erreurs accumulées (non fatales)
    errors:         Vec<TypeError>,
}

impl TypeChecker {
    pub fn new(program: &Program) -> Self {
        Self {
            classes:        program.classes   .iter().map(|c| (c.name.clone(), c.clone())).collect(),
            interfaces:     program.interfaces.iter().map(|i| (i.name.clone(), i.clone())).collect(),
            type_params:    HashSet::new(),
            current_class:  None,
            expected_return: Type::Void,
            errors:         vec![],
        }
    }

    // ── Rapport d'erreurs ──────────────────────────────────────────────────

    pub fn check(mut self, program: &Program) -> Vec<TypeError> {
        self.check_program(program);
        self.errors
    }

    fn err(&mut self, msg: String) {
        self.errors.push(TypeError(msg));
    }

    // ── Programme ─────────────────────────────────────────────────────────────

    fn check_program(&mut self, program: &Program) {
        self.check_class_hierarchy();
        self.check_interface_impls();

        for class in &program.classes.clone() {
            self.check_class(class);
        }

        let mut env = TypeEnv::new();
        self.expected_return = Type::Int;
        for stmt in &program.main.body.clone() {
            self.check_stmt(stmt, &mut env);
        }
    }

    // ── Hiérarchie : parents et interfaces existent, pas de cycle ─────────────

    fn check_class_hierarchy(&mut self) {
        let class_names: Vec<String> = self.classes.keys().cloned().collect();
        for name in &class_names {
            let class = self.classes[name].clone();

            // Parent existe ?
            if let Some(parent) = &class.parent {
                if !self.classes.contains_key(parent) {
                    self.err(format!("Classe '{}' extends '{}' inconnu", name, parent));
                }
            }

            // Interfaces existent ?
            for iface in &class.implements {
                if !self.interfaces.contains_key(iface) {
                    self.err(format!("Classe '{}' implements '{}' inconnu", name, iface));
                }
            }

            // Cycle d'héritage ?
            if self.has_inheritance_cycle(name) {
                self.err(format!("Cycle d'héritage détecté pour '{}'", name));
            }
        }
    }

    fn has_inheritance_cycle(&self, start: &str) -> bool {
        let mut visited = HashSet::new();
        let mut current = start.to_string();
        loop {
            if visited.contains(&current) { return true; }
            visited.insert(current.clone());
            match self.classes.get(&current).and_then(|c| c.parent.clone()) {
                Some(p) => current = p,
                None    => return false,
            }
        }
    }

    // ── Interfaces : méthodes présentes dans la classe ────────────────────────

    fn check_interface_impls(&mut self) {
        let class_names: Vec<String> = self.classes.keys().cloned().collect();
        for class_name in &class_names {
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

    // ── Classe ────────────────────────────────────────────────────────────────

    fn check_class(&mut self, class: &ClassDef) {
        self.current_class = Some(class.name.clone());
        self.type_params   = class.type_params.iter().cloned().collect();
        debug!("  check class '{}'", class.name);

        // Constructeurs
        for ctor in &class.constructors.clone() {
            let mut env = TypeEnv::new();
            env.push();
            for p in &ctor.params { env.declare(p.name.clone(), p.ty.clone()); }
            self.expected_return = Type::Void;
            for stmt in &ctor.body.clone() { self.check_stmt(stmt, &mut env); }
        }

        // Méthodes
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

    // ── Instruction ───────────────────────────────────────────────────────────

    fn check_stmt(&mut self, stmt: &Stmt, env: &mut TypeEnv) {
        match stmt {
            Stmt::VarDecl { ty, name, init } => {
                if let Some(expr) = init {
                    if let Ok(expr_ty) = self.infer_expr(expr, env) {
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
                let val_ty = self.infer_expr(value, env);
                // Cherche le type de la cible (variable locale ou champ de this)
                let target_ty = env.get(target)
                    .cloned()
                    .or_else(|| self.field_type_of_current_class(target));

                if let (Ok(vt), Some(tt)) = (val_ty, target_ty) {
                    if !self.is_compatible(&vt, &tt) {
                        self.err(format!("Affectation '{}' : {} incompatible avec {}", target, vt, tt));
                    }
                }
            }

            Stmt::FieldAssign { object, field, value } => {
                let val_ty = self.infer_expr(value, env);
                let obj_ty = if object == "this" {
                    self.current_class.as_ref().map(|c| Type::UserDefined(c.clone()))
                } else {
                    env.get(object).cloned()
                        .or_else(|| self.field_type_of_current_class(object))
                };

                if let Some(ot) = obj_ty {
                    match self.find_field_type_of(&ot, field) {
                        Some(ft) => {
                            if let Ok(vt) = val_ty {
                                if !self.is_compatible(&vt, &ft) {
                                    self.err(format!(
                                        "Affectation {}.{} : {} incompatible avec {}",
                                        object, field, vt, ft
                                    ));
                                }
                            }
                        }
                        None => {
                            // Avertissement seulement pour champs hérités / génériques
                            warn!("Champ inconnu '{}.{}' (peut être un champ hérité)", object, field);
                        }
                    }
                }
            }

            Stmt::Print(args) => {
                for a in args { let _ = self.infer_expr(a, env); }
            }

            Stmt::Return(expr) => {
                let ret = self.expected_return.clone();
                if let Some(e) = expr {
                    if let Ok(ty) = self.infer_expr(e, env) {
                        if !self.is_compatible(&ty, &ret) {
                            self.err(format!("return {} mais attendu {}", ty, ret));
                        }
                    }
                } else if ret != Type::Void {
                    self.err(format!("return vide mais type attendu {}", ret));
                }
            }

            Stmt::ExprStmt(e) => { let _ = self.infer_expr(e, env); }

            Stmt::If { condition, then_body, else_body } => {
                if let Ok(ct) = self.infer_expr(condition, env) {
                    if ct != Type::Bool {
                        self.err(format!("Condition du if doit être bool, trouvé {}", ct));
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
                if let Ok(ct) = self.infer_expr(condition, env) {
                    if ct != Type::Bool {
                        self.err(format!("Condition du while doit être bool, trouvé {}", ct));
                    }
                }
                env.push();
                for s in body { self.check_stmt(s, env); }
                env.pop();
            }

            Stmt::For { init, condition, update, body } => {
                env.push();
                if let Some(s) = init    { self.check_stmt(s, env); }
                if let Some(e) = condition {
                    if let Ok(ct) = self.infer_expr(e, env) {
                        if ct != Type::Bool {
                            self.err(format!("Condition du for doit être bool, trouvé {}", ct));
                        }
                    }
                }
                if let Some(s) = update  { self.check_stmt(s, env); }
                env.push();
                for s in body { self.check_stmt(s, env); }
                env.pop();
                env.pop();
            }

            Stmt::Break | Stmt::Continue => { /* toujours valide */ }
        }
    }

    // ── Inférence de type d'une expression ────────────────────────────────────

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
                        .unwrap_or(Type::Void));
                }
                // Type param → compatible avec tout
                if self.type_params.contains(name.as_str()) {
                    return Ok(Type::UserDefined(name.clone()));
                }
                env.get(name)
                    .cloned()
                    .or_else(|| self.field_type_of_current_class(name))
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

                // Nombre d'arguments
                if args.len() != param_types.len() {
                    return type_err!(
                        "{}() : {} arg(s) attendus, {} fournis",
                        method, param_types.len(), args.len()
                    );
                }

                // Types des arguments
                for (arg, param_ty) in args.iter().zip(param_types.iter()) {
                    let arg_ty = self.infer_expr(arg, env)?;
                    let expected = substitute(param_ty, &subst);
                    if !self.is_compatible(&arg_ty, &expected) {
                        self.err(format!(
                            "Argument de {}() : {} incompatible avec {}",
                            method, arg_ty, expected
                        ));
                    }
                }

                Ok(substitute(&return_type, &subst))
            }

            Expr::FunctionCall { name, args } => {
                // print est une pseudo-fonction builtin
                if name == "print" {
                    for a in args { let _ = self.infer_expr(a, env); }
                    return Ok(Type::Void);
                }

                // Cherche dans les méthodes de la classe courante
                if let Some(class_name) = self.current_class.clone() {
                    if let Some(m) = self.find_method_def(&class_name, name) {
                        let m = m.clone();
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

                type_err!("Fonction inconnue '{}'", name)
            }

            Expr::New { class_name, type_args, args } => {
                let class = self.classes.get(class_name)
                    .ok_or_else(|| TypeError(format!("Classe inconnue '{}'", class_name)))?
                    .clone();

                // Substitution des paramètres génériques
                let subst: Vec<(String, Type)> = class.type_params.iter()
                    .zip(type_args.iter())
                    .map(|(p, t)| (p.clone(), t.clone()))
                    .collect();

                if !class.constructors.is_empty() {
                    // Trouve un constructeur compatible par arité
                    let ctor = class.constructors.iter()
                        .find(|c| c.params.len() == args.len())
                        .ok_or_else(|| TypeError(format!(
                            "Pas de constructeur à {} arg(s) pour '{}'",
                            args.len(), class_name
                        )))?
                        .clone();

                    for (arg, p) in args.iter().zip(ctor.params.iter()) {
                        let arg_ty = self.infer_expr(arg, env)?;
                        let expected = substitute(&p.ty, &subst);
                        if !self.is_compatible(&arg_ty, &expected) {
                            self.err(format!(
                                "new {}() : arg {} incompatible avec {}",
                                class_name, arg_ty, expected
                            ));
                        }
                    }
                } else if !args.is_empty() {
                    self.err(format!(
                        "'{}' n'a pas de constructeur mais reçoit {} arg(s)",
                        class_name, args.len()
                    ));
                }

                if type_args.is_empty() {
                    Ok(Type::UserDefined(class_name.clone()))
                } else {
                    Ok(Type::Generic(class_name.clone(), type_args.clone()))
                }
            }
        }
    }

    // ── Compatibilité de types ────────────────────────────────────────────────

    /// Vérifie que `actual` est compatible avec `expected`.
    /// Primitifs : exact.  Classes : sous-type.  Type param : toujours ok.
    pub fn is_compatible(&self, actual: &Type, expected: &Type) -> bool {
        // Paramètre générique → compatible avec tout
        if let Type::UserDefined(n) = actual   { if self.type_params.contains(n) { return true; } }
        if let Type::UserDefined(n) = expected { if self.type_params.contains(n) { return true; } }

        // Même type exact
        if actual == expected { return true; }

        // Null / Void
        if matches!(actual, Type::Void) { return true; }

        // Promotions numériques
        if matches!(expected, Type::Double) && self.is_numeric(actual) { return true; }
        if matches!(expected, Type::Float)  && matches!(actual, Type::Int) { return true; }

        // Sous-typage de classes
        if let (Type::UserDefined(a), Type::UserDefined(e)) = (actual, expected) {
            return self.is_subclass(a, e);
        }

        // Generic<T> compatible avec même générique ou parent
        if let (Type::Generic(an, _), Type::UserDefined(en)) = (actual, expected) {
            return self.is_subclass(an, en);
        }
        if let (Type::UserDefined(an), Type::Generic(en, _)) = (actual, expected) {
            return self.is_subclass(an, en);
        }
        if let (Type::Generic(an, aa), Type::Generic(en, ea)) = (actual, expected) {
            return an == en && aa.len() == ea.len()
                && aa.iter().zip(ea.iter()).all(|(a, e)| self.is_compatible(a, e));
        }

        false
    }

    fn is_subclass(&self, sub: &str, sup: &str) -> bool {
        if sub == sup { return true; }
        if let Some(class) = self.classes.get(sub) {
            if let Some(parent) = &class.parent {
                return self.is_subclass(parent, sup);
            }
        }
        false
    }

    // ── Opérateurs binaires ───────────────────────────────────────────────────

    fn check_binop(&mut self, lt: &Type, op: &BinOp, rt: &Type) -> Result<Type, TypeError> {
        match op {
            BinOp::Add => {
                if self.is_numeric(lt) && self.is_numeric(rt) {
                    Ok(self.numeric_result(lt, rt))
                } else if matches!((lt, rt), (Type::Str, Type::Str)) {
                    Ok(Type::Str)
                } else {
                    type_err!("Opérateur + non applicable à {} et {}", lt, rt)
                }
            }
            BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow => {
                if self.is_numeric(lt) && self.is_numeric(rt) {
                    Ok(self.numeric_result(lt, rt))
                } else {
                    type_err!("Opérateur arithmétique non applicable à {} et {}", lt, rt)
                }
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                if self.is_numeric(lt) && self.is_numeric(rt) {
                    Ok(Type::Bool)
                } else {
                    type_err!("Comparaison non applicable à {} et {}", lt, rt)
                }
            }
            BinOp::Eq | BinOp::Ne => Ok(Type::Bool),
            BinOp::And | BinOp::Or => {
                if matches!((lt, rt), (Type::Bool, Type::Bool)) {
                    Ok(Type::Bool)
                } else {
                    type_err!("Opérateurs logiques requièrent bool, trouvé {} et {}", lt, rt)
                }
            }
        }
    }

    fn is_numeric(&self, t: &Type) -> bool {
        matches!(t, Type::Int | Type::Float | Type::Double)
    }

    fn numeric_result(&self, lt: &Type, rt: &Type) -> Type {
        match (lt, rt) {
            (Type::Double, _) | (_, Type::Double) => Type::Double,
            (Type::Float,  _) | (_, Type::Float)  => Type::Float,
            _                                      => Type::Int,
        }
    }

    // ── Résolution de méthode ─────────────────────────────────────────────────

    /// Retourne (params, return_type, substitution) pour une méthode sur un type.
    fn resolve_method(
        &self,
        obj_ty: &Type,
        method: &str,
    ) -> Result<(Vec<Type>, Type, Vec<(String, Type)>), TypeError> {
        let (class_name, type_args) = match obj_ty {
            Type::UserDefined(n)   => (n.clone(), vec![]),
            Type::Generic(n, args) => (n.clone(), args.clone()),
            _ => return type_err!("Appel de méthode '{}' sur type non-objet {}", method, obj_ty),
        };

        let m = self.find_method_def(&class_name, method)
            .ok_or_else(|| TypeError(format!(
                "Méthode '{}' inconnue dans '{}'", method, class_name
            )))?
            .clone();

        let class = self.classes.get(&class_name)
            .ok_or_else(|| TypeError(format!("Classe '{}' inconnue", class_name)))?;

        let subst: Vec<(String, Type)> = class.type_params.iter()
            .zip(type_args.iter())
            .map(|(p, t)| (p.clone(), t.clone()))
            .collect();

        let param_types: Vec<Type> = m.params.iter().map(|p| p.ty.clone()).collect();
        Ok((param_types, m.return_type.clone(), subst))
    }

    // ── Lookup dans la hiérarchie de classes ──────────────────────────────────

    fn find_method_def<'a>(&'a self, class_name: &str, method_name: &str) -> Option<&'a Method> {
        let class = self.classes.get(class_name)?;
        if let Some(m) = class.methods.iter().find(|m| m.name == method_name) {
            return Some(m);
        }
        if let Some(parent) = &class.parent {
            return self.find_method_def(parent, method_name);
        }
        None
    }

    fn find_field_type_of(&self, obj_ty: &Type, field: &str) -> Option<Type> {
        let class_name = match obj_ty {
            Type::UserDefined(n) | Type::Generic(n, _) => n.clone(),
            _ => return None,
        };
        self.find_field_in_class(&class_name, field)
    }

    fn find_field_in_class(&self, class_name: &str, field: &str) -> Option<Type> {
        let class = self.classes.get(class_name)?;
        if let Some(f) = class.fields.iter().find(|f| f.name == field) {
            return Some(f.ty.clone());
        }
        if let Some(parent) = &class.parent {
            return self.find_field_in_class(parent, field);
        }
        None
    }

    fn field_type_of_current_class(&self, field: &str) -> Option<Type> {
        let class_name = self.current_class.as_ref()?;
        self.find_field_in_class(class_name, field)
    }
}
