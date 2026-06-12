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
    scopes:      Vec<HashMap<String, Type>>,
    qual_scopes: Vec<HashMap<String, Qualifier>>,
}

impl TypeEnv {
    fn new() -> Self { Self { scopes: vec![HashMap::new()], qual_scopes: vec![HashMap::new()] } }
    fn push(&mut self) { self.scopes.push(HashMap::new()); self.qual_scopes.push(HashMap::new()); }
    fn pop(&mut self)  {
        if self.scopes.len() > 1 { self.scopes.pop(); self.qual_scopes.pop(); }
    }

    fn declare(&mut self, name: String, ty: Type) {
        self.scopes.last_mut().unwrap().insert(name.clone(), ty);
        self.qual_scopes.last_mut().unwrap().insert(name, Qualifier::Mutable);
    }
    fn declare_qualified(&mut self, name: String, ty: Type, qual: Qualifier) {
        self.scopes.last_mut().unwrap().insert(name.clone(), ty);
        self.qual_scopes.last_mut().unwrap().insert(name, qual);
    }
    fn get(&self, name: &str) -> Option<&Type> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }
    fn get_qualifier(&self, name: &str) -> Qualifier {
        self.qual_scopes.iter().rev()
            .find_map(|s| s.get(name))
            .cloned()
            .unwrap_or(Qualifier::Mutable)
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
    aliases:                HashMap<String, Type>,
    funcs:                  HashMap<String, FuncDef>,
    modules:                Vec<ModuleDef>,
    /// Bindings explicites issus des modules : interface → service concret
    binds_to:               HashMap<String, String>,
    /// Valeurs de configuration issues des modules : service → args du `with`
    with_values:            HashMap<String, Vec<Expr>>,
    type_params:            HashSet<String>,
    current_class:          Option<String>,
    current_enum:           Option<String>,
    expected_return:        Type,
    /// true si la méthode courante est déclarée `mutable`
    current_method_mutable: bool,
    errors:                 Vec<TypeError>,
}

impl TypeChecker {
    // ── Conversions record → ClassDef ─────────────────────────────────────────

    /// Capitalise la première lettre d'un identifiant : `"x"` → `"X"`.
    fn capitalize(s: &str) -> String {
        let mut c = s.chars();
        match c.next() {
            None    => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }

    /// Convertit un `RecordDef` en `ClassDef` en synthétisant les méthodes
    /// générées automatiquement (getters, copy, equals, toString, hashCode).
    /// Version publique pour que l'interpréteur puisse aussi l'utiliser.
    pub fn record_to_class_pub(rd: &RecordDef) -> ClassDef { Self::record_to_class(rd) }

    fn record_to_class(rd: &RecordDef) -> ClassDef {
        let mut methods: Vec<Method> = vec![];

        // ── Getters — corps réel pour que l'interpréteur puisse les exécuter
        for field in &rd.fields {
            methods.push(Method {
                visibility:  Visibility::Public,
                is_mutable:  false,
                return_type: field.ty.clone(),
                name:        format!("get{}", Self::capitalize(&field.name)),
                params:      vec![],
                // `return fieldName;` — le champ est accessible via this dans l'interpréteur
                body:        vec![Stmt::Return(Some(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("this".to_string())),
                    field:  field.name.clone(),
                }))],
            });
        }

        // ── copy(Option<T1> f1, Option<T2> f2, ...) ───────────────────────
        let copy_params: Vec<Param> = rd.fields.iter().map(|f| Param {
            name: f.name.clone(),
            ty:   Type::Generic("Option".to_string(), vec![f.ty.clone()]),
        }).collect();
        let copy_return = if rd.type_params.is_empty() {
            Type::UserDefined(rd.name.clone())
        } else {
            Type::Generic(
                rd.name.clone(),
                rd.type_params.iter().map(|p| Type::UserDefined(p.clone())).collect(),
            )
        };
        methods.push(Method {
            visibility:  Visibility::Public,
            is_mutable:  false,
            return_type: copy_return,
            name:        "copy".to_string(),
            params:      copy_params,
            body:        vec![Stmt::Builtin],
        });

        // ── toString() ────────────────────────────────────────────────────
        methods.push(Method {
            visibility:  Visibility::Public,
            is_mutable:  false,
            return_type: Type::Str,
            name:        "toString".to_string(),
            params:      vec![],
            body:        vec![Stmt::Builtin],
        });

        // ── equals(Object) ────────────────────────────────────────────────
        methods.push(Method {
            visibility:  Visibility::Public,
            is_mutable:  false,
            return_type: Type::Bool,
            name:        "equals".to_string(),
            params:      vec![Param { name: "other".to_string(),
                                      ty:   Type::UserDefined("Object".to_string()) }],
            body:        vec![Stmt::Builtin],
        });

        // ── hashCode() ────────────────────────────────────────────────────
        methods.push(Method {
            visibility:  Visibility::Public,
            is_mutable:  false,
            return_type: Type::Int,
            name:        "hashCode".to_string(),
            params:      vec![],
            body:        vec![Stmt::Builtin],
        });

        // ── Méthodes custom de l'utilisateur ──────────────────────────────
        methods.extend(rd.methods.clone());

        // ── Constructeur implicite : `this.fieldN = fieldN;` pour chaque champ
        let ctor_body: Vec<Stmt> = rd.fields.iter()
            .map(|f| Stmt::FieldAssign {
                object: "this".to_string(),
                field:  f.name.clone(),
                value:  Expr::Ident(f.name.clone()),
            })
            .collect();
        let constructor = Constructor {
            params: rd.fields.iter()
                .map(|f| Param { name: f.name.clone(), ty: f.ty.clone() })
                .collect(),
            body: ctor_body,
        };

        ClassDef {
            is_service:             false,
            is_transient:           false,
            is_mut:                 true,
            name:                   rd.name.clone(),
            type_params:            rd.type_params.clone(),
            type_param_constraints: rd.type_param_constraints.clone(),
            parent:                 Some("Record".to_string()),
            implements:             rd.implements.clone(),
            fields:                 rd.fields.clone(),
            constructors:           vec![constructor],
            methods,
        }
    }

    pub fn new(program: &Program) -> Self {
        let mut classes: HashMap<String, ClassDef> =
            program.classes.iter().map(|c| (c.name.clone(), c.clone())).collect();
        classes.entry("Object".to_string()).or_insert_with(|| ClassDef {
            is_service: false,
            is_transient: false,
            is_mut: true,
            name: "Object".to_string(),
            type_params: vec![],
            type_param_constraints: vec![],
            parent: None,
            implements: vec![],
            fields: vec![],
            constructors: vec![],
            methods: vec![Method {
                visibility:  Visibility::Public,
                is_mutable:  false,
                return_type: Type::Bool,
                name: "equals".to_string(),
                params: vec![Param { name: "other".to_string(), ty: Type::UserDefined("Object".to_string()) }],
                body: vec![Stmt::Builtin],
            }],
        });
        // ── Classe abstraite Record — ancêtre de tous les records ─────────
        classes.entry("Record".to_string()).or_insert_with(|| ClassDef {
            is_service: false,
            is_transient: false,
            is_mut: true,
            name: "Record".to_string(),
            type_params: vec![],
            type_param_constraints: vec![],
            parent: Some("Object".to_string()),
            implements: vec![],
            fields: vec![],
            constructors: vec![],
            methods: vec![
                Method {
                    visibility: Visibility::Public, is_mutable: false,
                    return_type: Type::Bool, name: "equals".to_string(),
                    params: vec![Param { name: "other".to_string(), ty: Type::UserDefined("Object".to_string()) }],
                    body: vec![Stmt::Builtin],
                },
                Method {
                    visibility: Visibility::Public, is_mutable: false,
                    return_type: Type::Str, name: "toString".to_string(),
                    params: vec![], body: vec![Stmt::Builtin],
                },
                Method {
                    visibility: Visibility::Public, is_mutable: false,
                    return_type: Type::Int, name: "hashCode".to_string(),
                    params: vec![], body: vec![Stmt::Builtin],
                },
            ],
        });
        // ── Convertit chaque record en ClassDef ───────────────────────────
        for rd in &program.records {
            classes.insert(rd.name.clone(), Self::record_to_class(rd));
        }
        Self {
            classes,
            interfaces:      program.interfaces.iter().map(|i| (i.name.clone(), i.clone())).collect(),
            enums:           program.enums.iter().map(|e| (e.name.clone(), e.clone())).collect(),
            aliases:         program.type_aliases.iter().map(|a| (a.name.clone(), a.ty.clone())).collect(),
            funcs:           program.funcs.iter().map(|f| (f.name.clone(), f.clone())).collect(),
            modules:         program.modules.clone(),
            binds_to:        HashMap::new(),
            with_values:     HashMap::new(),
            type_params:            HashSet::new(),
            current_class:          None,
            current_enum:           None,
            expected_return:        Type::Void,
            current_method_mutable: true,
            errors:                 vec![],
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
        self.check_modules();
        self.check_services();
        self.check_enums(program);
        self.check_records(program);
        for class in &program.classes.clone() { self.check_class(class); }
        for func in &program.funcs.clone() { self.check_func(func); }
        let mut env = TypeEnv::new();
        self.expected_return = Type::Int;
        for stmt in &program.main.body.clone() { self.check_stmt(stmt, &mut env); }
    }

    // ── Records ───────────────────────────────────────────────────────────────

    fn check_records(&mut self, program: &Program) {
        for rd in &program.records.clone() {
            // Interdire les méthodes mutable dans un record
            for m in &rd.methods {
                if m.is_mutable {
                    self.err(format!(
                        "Record '{}' : la méthode '{}' ne peut pas être mutable \
                         (les records sont immuables)",
                        rd.name, m.name));
                }
                if m.visibility != Visibility::Public {
                    self.err(format!(
                        "Record '{}' : la méthode '{}' doit être publique \
                         (les méthodes privées/protected ne sont pas autorisées dans un record)",
                        rd.name, m.name));
                }
            }
            // Vérifier les interfaces implémentées
            for iname in &rd.implements.clone() {
                if !self.interfaces.contains_key(iname) {
                    self.err(format!("Record '{}' implements '{}' inconnu", rd.name, iname));
                } else if let Some(iface) = self.interfaces.get(iname).cloned() {
                    for sig in &iface.methods {
                        let synth = Self::record_to_class(rd);
                        if synth.methods.iter().find(|m| m.name == sig.name).is_none() {
                            self.err(format!(
                                "Record '{}' n'implémente pas '{}.{}()'",
                                rd.name, iname, sig.name));
                        }
                    }
                }
            }
            // Typecheck des corps de méthodes custom via check_class
            let class_def = Self::record_to_class(rd);
            self.check_class(&class_def);
        }
    }

    fn check_func(&mut self, func: &FuncDef) {
        let mut env = TypeEnv::new();
        for p in &func.params { env.declare(p.name.clone(), p.ty.clone()); }
        let saved = self.expected_return.clone();
        self.expected_return = func.return_type.clone();
        for stmt in &func.body.clone() { self.check_stmt(stmt, &mut env); }
        self.expected_return = saved;
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

    // ── Injection de dépendances ──────────────────────────────────────────────
    //
    //  Les classes `service` sont instanciées par le conteneur d'injection :
    //  leurs dépendances sont les paramètres de leur constructeur. Tout est
    //  validé ici, au typecheck — l'exécution ne peut pas échouer :
    //  - un service a au plus un constructeur et n'est pas générique ;
    //  - chaque dépendance résout vers exactement un service (binding explicite
    //    d'un module, sinon implémentation unique) ;
    //  - les paramètres non-service sont couverts par un `bind ... with (...)` ;
    //  - un singleton ne dépend pas d'un service `transient` ;
    //  - le graphe de dépendances est acyclique.

    /// true si le paramètre est un slot de dépendance (injecté par le conteneur) :
    /// son type est une interface ou une classe `service`. Tout autre type est
    /// un slot de configuration, à fournir via `bind ... with (...)`.
    /// L'interpréteur applique exactement la même classification.
    fn is_dep_slot(&self, ty: &Type) -> bool {
        match ty {
            Type::UserDefined(n) =>
                self.interfaces.contains_key(n)
                || self.classes.get(n).map(|c| c.is_service).unwrap_or(false),
            _ => false,
        }
    }

    /// Résout un nom de type injectable vers le nom de la classe service concrète.
    /// - classe `service`                    → elle-même ;
    /// - interface liée par un module        → le service du `bind` ;
    /// - interface implémentée par exactement un service → ce service ;
    /// - sinon                               → erreur (binding manquant ou ambigu).
    fn resolve_service(&self, name: &str) -> Result<String, TypeError> {
        if let Some(c) = self.classes.get(name) {
            if c.is_service { return Ok(name.to_string()); }
            return type_err!(
                "'{}' n'est pas injectable : la classe doit être déclarée `service`", name);
        }
        if self.interfaces.contains_key(name) {
            // Binding explicite d'un module — prioritaire sur la règle d'unicité
            if let Some(s) = self.binds_to.get(name) { return Ok(s.clone()); }
            let mut impls: Vec<String> = self.classes.values()
                .filter(|c| c.is_service && c.implements.iter().any(|i| i == name))
                .map(|c| c.name.clone())
                .collect();
            impls.sort();
            return match impls.len() {
                0 => type_err!(
                    "Aucun service n'implémente l'interface '{}'", name),
                1 => Ok(impls.remove(0)),
                _ => type_err!(
                    "Binding ambigu : '{}' est implémenté par plusieurs services ({}) \
                     — choisissez avec `bind {} to ...` dans un module",
                    name, impls.join(", "), name),
            };
        }
        type_err!("'{}' n'est pas injectable : ni classe `service` ni interface", name)
    }

    /// Dépendances d'un service = paramètres-dépendances de son constructeur,
    /// résolus vers les classes services concrètes. Les dépendances non
    /// résolvables sont ignorées ici (l'erreur est déjà remontée par check_services).
    fn service_deps(&self, cn: &str) -> Vec<String> {
        self.classes.get(cn)
            .and_then(|c| c.constructors.first())
            .map(|ctor| ctor.params.iter()
                .filter(|p| self.is_dep_slot(&p.ty))
                .filter_map(|p| match &p.ty {
                    Type::UserDefined(n) => self.resolve_service(n).ok(),
                    _ => None,
                })
                .collect())
            .unwrap_or_default()
    }

    /// Valide les modules et construit les tables `binds_to` / `with_values`.
    /// Plusieurs modules peuvent coexister : leurs bindings sont fusionnés,
    /// un binding (ou un `with`) dupliqué pour la même cible est une erreur.
    fn check_modules(&mut self) {
        for m in &self.modules.clone() {
            for b in &m.binds {
                let target_is_iface   = self.interfaces.contains_key(&b.target);
                let target_is_service = self.classes.get(&b.target)
                    .map(|c| c.is_service).unwrap_or(false);
                if !target_is_iface && !target_is_service {
                    self.err(format!(
                        "Module '{}' : bind '{}' — '{}' n'est ni une interface ni une \
                         classe service", m.name, b.target, b.target));
                    continue;
                }

                // ── bind Iface to Service ─────────────────────────────────
                let concrete: Option<String> = match &b.to {
                    Some(to) => {
                        if !target_is_iface {
                            self.err(format!(
                                "Module '{}' : bind {} to {} — la cible d'un `to` doit \
                                 être une interface", m.name, b.target, to));
                            None
                        } else {
                            match self.classes.get(to) {
                                None => {
                                    self.err(format!(
                                        "Module '{}' : bind {} to {} — classe '{}' inconnue",
                                        m.name, b.target, to, to));
                                    None
                                }
                                Some(c) if !c.is_service => {
                                    self.err(format!(
                                        "Module '{}' : bind {} to {} — '{}' doit être \
                                         déclarée `service`", m.name, b.target, to, to));
                                    None
                                }
                                Some(c) if !c.implements.iter().any(|i| i == &b.target) => {
                                    self.err(format!(
                                        "Module '{}' : bind {} to {} — '{}' n'implémente \
                                         pas '{}'", m.name, b.target, to, to, b.target));
                                    None
                                }
                                Some(_) => {
                                    if self.binds_to.contains_key(&b.target) {
                                        self.err(format!(
                                            "Binding dupliqué pour '{}' : déjà lié à '{}'",
                                            b.target, self.binds_to[&b.target]));
                                        None
                                    } else {
                                        self.binds_to.insert(b.target.clone(), to.clone());
                                        Some(to.clone())
                                    }
                                }
                            }
                        }
                    }
                    None if target_is_service => Some(b.target.clone()),
                    None => {
                        // Interface sans `to`
                        if b.with.is_empty() {
                            self.err(format!(
                                "Module '{}' : bind {} — binding sans effet (précisez \
                                 `to` et/ou `with`)", m.name, b.target));
                        } else {
                            self.err(format!(
                                "Module '{}' : bind {} with (...) — `with` sur une \
                                 interface nécessite `to`", m.name, b.target));
                        }
                        None
                    }
                };

                // ── with (valeurs de configuration) ───────────────────────
                if !b.with.is_empty() {
                    if let Some(c) = concrete {
                        if self.with_values.contains_key(&c) {
                            self.err(format!(
                                "Valeurs `with` dupliquées pour le service '{}'", c));
                        } else {
                            self.with_values.insert(c, b.with.clone());
                        }
                    }
                } else if b.to.is_none() && target_is_service {
                    self.err(format!(
                        "Module '{}' : bind {} — binding sans effet (précisez \
                         `to` et/ou `with`)", m.name, b.target));
                }
            }
        }
    }

    fn check_services(&mut self) {
        // `transient` sans `service` est interdit
        for c in self.classes.values().cloned().collect::<Vec<_>>() {
            if c.is_transient && !c.is_service {
                self.err(format!(
                    "Classe '{}' : `transient` nécessite `service`", c.name));
            }
        }

        let mut services: Vec<String> = self.classes.values()
            .filter(|c| c.is_service)
            .map(|c| c.name.clone())
            .collect();
        services.sort();

        // ── Validation de chaque service ──────────────────────────────────
        for sn in &services {
            let class = self.classes[sn].clone();
            if !class.type_params.is_empty() {
                self.err(format!(
                    "Service '{}' : un service ne peut pas être générique", sn));
            }
            if class.constructors.len() > 1 {
                self.err(format!(
                    "Service '{}' : un service doit avoir au plus un constructeur \
                     ({} trouvés)", sn, class.constructors.len()));
            }
            // Partition des paramètres : dépendances vs configuration
            let mut config_params: Vec<Param> = vec![];
            if let Some(ctor) = class.constructors.first() {
                for p in &ctor.params {
                    if self.is_dep_slot(&p.ty) {
                        let dep = match &p.ty {
                            Type::UserDefined(n) => n.clone(),
                            _ => unreachable!("is_dep_slot ne matche que UserDefined"),
                        };
                        match self.resolve_service(&dep) {
                            Err(e) => self.err(format!(
                                "Service '{}', dépendance '{}' : {}", sn, p.name, e.0)),
                            Ok(concrete) => {
                                // Dépendance captive : un singleton qui capture un
                                // transient figerait son instance — interdit.
                                let dep_transient = self.classes.get(&concrete)
                                    .map(|c| c.is_transient).unwrap_or(false);
                                if !class.is_transient && dep_transient {
                                    self.err(format!(
                                        "Service '{}' (singleton) ne peut pas dépendre du \
                                         service transient '{}' (dépendance captive : \
                                         l'instance serait figée)", sn, concrete));
                                }
                            }
                        }
                    } else {
                        config_params.push(p.clone());
                    }
                }
            }
            // Paramètres de configuration : couverts par `bind ... with (...)` ?
            let with = self.with_values.get(sn).cloned().unwrap_or_default();
            if with.len() != config_params.len() {
                if with.is_empty() {
                    for p in &config_params {
                        self.err(format!(
                            "Service '{}' : le paramètre '{}' de type {} n'est pas \
                             injectable — fournissez sa valeur via `bind {} with (...)` \
                             dans un module", sn, p.name, p.ty, sn));
                    }
                } else {
                    self.err(format!(
                        "bind {} with : {} valeur(s) fournie(s) mais {} paramètre(s) \
                         de configuration dans le constructeur",
                        sn, with.len(), config_params.len()));
                }
            } else {
                let env = TypeEnv::new();
                for (expr, p) in with.iter().zip(config_params.iter()) {
                    if let Some(at) = self.infer(expr, &env) {
                        let expected = self.resolve(&p.ty);
                        if !self.is_compatible(&at, &expected) {
                            self.err(format!(
                                "bind {} with : type incompatible pour '{}' : {} ≠ {}",
                                sn, p.name, at, expected));
                        }
                    }
                }
            }
        }

        // ── Détection de cycle (DFS avec chemin courant) ──────────────────
        let mut done: HashSet<String> = HashSet::new();
        for sn in &services {
            if done.contains(sn) { continue; }
            let mut path: Vec<String> = vec![];
            if let Some(cycle) = self.find_service_cycle(sn, &mut path, &mut done) {
                self.err(format!("Cycle de dépendances entre services : {}", cycle));
            }
        }
    }

    /// DFS : retourne la description du cycle si un service du chemin courant
    /// est revisité. `done` évite de re-parcourir (et re-signaler) les sous-graphes.
    fn find_service_cycle(
        &self, cur: &str, path: &mut Vec<String>, done: &mut HashSet<String>,
    ) -> Option<String> {
        if let Some(pos) = path.iter().position(|p| p == cur) {
            let mut cycle: Vec<&str> = path[pos..].iter().map(|s| s.as_str()).collect();
            cycle.push(cur);
            return Some(cycle.join(" → "));
        }
        if done.contains(cur) { return None; }
        path.push(cur.to_string());
        for dep in self.service_deps(cur) {
            if let Some(c) = self.find_service_cycle(&dep, path, done) {
                return Some(c);
            }
        }
        path.pop();
        done.insert(cur.to_string());
        None
    }

    // ── Enums ─────────────────────────────────────────────────────────────────

    fn check_enums(&mut self, program: &Program) {
        for ed in &program.enums.clone() {
            self.current_enum = Some(ed.name.clone());
            self.type_params  = ed.type_params.iter().cloned().collect();
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
            self.type_params  = HashSet::new();
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
            self.expected_return        = self.resolve(&m.return_type);
            self.current_method_mutable = m.is_mutable;
            for s in &m.body.clone() { self.check_stmt(s, &mut env); }
        }
        self.current_class          = None;
        self.current_method_mutable = true;
        self.type_params            = HashSet::new();
    }

    // ── Instructions ──────────────────────────────────────────────────────────

    fn check_stmt(&mut self, stmt: &Stmt, env: &mut TypeEnv) {
        match stmt {

            Stmt::VarDecl { qualifier, ty, name, init } => {
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
                // Vérifier que le type est `mut` si le qualificateur l'exige
                if *qualifier != Qualifier::Mutable {
                    self.check_type_is_mut(&resolved, name, qualifier);
                }
                // Vérifier les contraintes de type params sur le type déclaré
                if let Type::Generic(gname, gargs) = &resolved {
                    let (gname, gargs) = (gname.clone(), gargs.clone());
                    self.check_generic_type_constraints(&gname, &gargs);
                }
                env.declare_qualified(name.clone(), resolved, qualifier.clone());
            }

            Stmt::Assign { target, value } => {
                let vt = self.infer(value, env);
                let tt = env.get(target).cloned()
                    .or_else(|| self.field_of_current_class(target))
                    .map(|t| self.resolve(&t));
                if let (Some(vt), Some(tt)) = (vt, tt) {
                    if !self.is_compatible(&vt, &tt) {
                        self.err(format!("Affectation '{}' : type incompatible {} ≠ {}", target, vt, tt));
                    }
                }
            }

            Stmt::FieldAssign { object, field, value } => {
                let vt = self.infer(value, env);
                let via_this = object == "this";
                let ot = if via_this {
                    Some(self.this_type())
                } else {
                    env.get(object).cloned().or_else(|| self.field_of_current_class(object))
                };
                if let Some(ot) = ot {
                    let ot = self.resolve(&ot);
                    // Champs toujours privés — seul this peut y accéder
                    self.check_field_access_visibility(&ot, field, via_this);
                    match self.find_field_type(&ot, field) {
                        Some(ft) => {
                            if let Some(vt) = vt {
                                if !self.is_compatible(&vt, &ft) {
                                    self.err(format!("{}.{} : type incompatible {} ≠ {}", object, field, vt, ft));
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

            Stmt::ForIn { var_type, var_name, body, .. } => {
                env.push();
                env.declare(var_name.clone(), var_type.clone());
                for s in body { self.check_stmt(s, env); }
                env.pop();
            }

            Stmt::Builtin => { /* implémentation native — no-op */ }

            Stmt::Break | Stmt::Continue => {}

            Stmt::Match { expr, arms } => {
                let st = self.infer(expr, env);
                let (enum_name, subst): (Option<String>, Vec<(String, Type)>) =
                    match st.as_ref() {
                        Some(Type::UserDefined(n)) => (Some(n.clone()), vec![]),
                        Some(Type::Generic(n, ta)) => {
                            let s = self.enums.get(n).map(|ed| {
                                ed.type_params.iter().zip(ta.iter())
                                    .map(|(p, t)| (p.clone(), t.clone())).collect()
                            }).unwrap_or_default();
                            (Some(n.clone()), s)
                        }
                        _ => (None, vec![]),
                    };
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
                                        env.declare(b.clone(), substitute(&self.resolve(&f.ty), &subst));
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
            Expr::CharLit(_)   => Ok(Type::Char),

            Expr::Ident(name) => {
                if name == "this" {
                    return Ok(self.this_type());
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
                let via_this = matches!(object.as_ref(), Expr::Ident(n) if n == "this");
                self.check_field_access_visibility(&ot, field, via_this);
                self.find_field_type(&ot, field)
                    .ok_or_else(|| TypeError(format!("Champ inconnu '{}.{}'", ot, field)))
            }

            Expr::MethodCall { object, method, args } => {
                let ot = self.infer_expr(object, env)?;
                let ot = self.resolve(&ot);
                let (ptys, rty, subst) = self.resolve_method(&ot, method)?;

                // ── Vérification de visibilité ────────────────────────────────
                self.check_method_visibility(&ot, method);

                // ── Vérification d'immutabilité ───────────────────────────────
                if self.method_is_mutable(&ot, method) {
                    let qual = self.qualifier_of_expr(object, env);
                    match qual {
                        Qualifier::Readonly  =>
                            self.err(format!(
                                "Appel de méthode mutable '{}()' sur une variable readonly",
                                method)),
                        Qualifier::Immutable =>
                            self.err(format!(
                                "Appel de méthode mutable '{}()' sur une variable immutable",
                                method)),
                        Qualifier::Mutable => {}
                    }
                    // this dans une méthode non-mutable est readonly
                    if matches!(object.as_ref(), Expr::Ident(n) if n == "this")
                        && !self.current_method_mutable
                    {
                        self.err(format!(
                            "Méthode non-mutable : impossible d'appeler '{}()' (mutable) sur this",
                            method));
                    }
                }

                if args.len() != ptys.len() {
                    return type_err!("{}() : {} arg(s) attendus, {} fournis",
                        method, ptys.len(), args.len());
                }
                for (arg, pt) in args.iter().zip(ptys.iter()) {
                    if let Ok(at) = self.infer_expr(arg, env) {
                        let expected = substitute(pt, &subst);
                        if !self.is_compatible(&at, &expected) {
                            self.err(format!("Arg de {}() : type incompatible {} ≠ {}", method, at, expected));
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
                if name == "panic" {
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
                                        self.err(format!("Arg de {}() : type incompatible {} ≠ {}", name, at, pt));
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
                                    self.err(format!("Arg de {}() : type incompatible {} ≠ {}", name, at, p.ty));
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

                // Fonction de haut niveau
                if let Some(f) = self.funcs.get(name.as_str()).cloned() {
                    if args.len() != f.params.len() {
                        return type_err!("{}() : {} arg(s) attendus, {} fournis",
                            name, f.params.len(), args.len());
                    }
                    for (arg, p) in args.iter().zip(f.params.iter()) {
                        if let Ok(at) = self.infer_expr(arg, env) {
                            if !self.is_compatible(&at, &p.ty) {
                                self.err(format!("Arg de {}() : incompatible {} ≠ {}", name, at, p.ty));
                            }
                        }
                    }
                    return Ok(self.resolve(&f.return_type));
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
                                self.err(format!("new {}() : type incompatible {} ≠ {}", class_name, at, expected));
                            }
                        }
                    }
                } else if !args.is_empty() {
                    self.err(format!("'{}' n'a pas de constructeur", class_name));
                }
                // Vérifier les contraintes de type params à la construction
                if !type_args.is_empty() {
                    let cn = class_name.clone();
                    let ta = type_args.clone();
                    self.check_generic_type_constraints(&cn, &ta);
                }
                if type_args.is_empty() { Ok(Type::UserDefined(class_name.clone())) }
                else { Ok(Type::Generic(class_name.clone(), type_args.clone())) }
            }

            // ── inject T — point d'entrée du conteneur d'injection ──────────
            Expr::Inject(ty) => {
                if self.current_class.is_some() || self.current_enum.is_some() {
                    return type_err!(
                        "'inject' n'est autorisé que dans main ou les fonctions de \
                         haut niveau (point de composition unique)");
                }
                let name = match ty {
                    Type::UserDefined(n) => n.clone(),
                    other => return type_err!("inject : type non injectable {}", other),
                };
                // Vérifie que le binding résout (exactement un service)
                self.resolve_service(&name)?;
                Ok(Type::UserDefined(name))
            }

            Expr::EnumConstructor { enum_name, type_args, variant, args } => {
                let ed = self.enums.get(enum_name)
                    .ok_or_else(|| TypeError(format!("Enum inconnu '{}'", enum_name)))?.clone();
                if !ed.type_params.is_empty() && !type_args.is_empty()
                    && type_args.len() != ed.type_params.len()
                {
                    return type_err!("'{}' attend {} paramètre(s) de type, {} fourni(s)",
                        enum_name, ed.type_params.len(), type_args.len());
                }
                let subst: Vec<(String, Type)> = ed.type_params.iter()
                    .zip(type_args.iter())
                    .map(|(p, t)| (p.clone(), t.clone()))
                    .collect();
                let vd = ed.variants.iter().find(|v| &v.name == variant)
                    .ok_or_else(|| TypeError(format!(
                        "Variante '{}' inconnue dans '{}'", variant, enum_name)))?.clone();
                if args.len() != vd.fields.len() {
                    return type_err!("'{}::{}' : {} champ(s), {} fourni(s)",
                        enum_name, variant, vd.fields.len(), args.len());
                }
                for (arg, f) in args.iter().zip(vd.fields.iter()) {
                    if let Ok(at) = self.infer_expr(arg, env) {
                        let expected = substitute(&f.ty, &subst);
                        if !self.is_compatible(&at, &expected) {
                            self.err(format!("Champ '{}' de '{}::{}' : type incompatible {} ≠ {}",
                                f.name, enum_name, variant, at, expected));
                        }
                    }
                }
                if type_args.is_empty() { Ok(Type::UserDefined(enum_name.clone())) }
                else { Ok(Type::Generic(enum_name.clone(), type_args.clone())) }
            }

            // ── Navigation sûre  ?.field  et  ?.method() ─────────────────────

            Expr::SafeFieldAccess { object, field } => {
                let ot = self.infer_expr(object, env)?;
                let ot = self.resolve(&ot);
                let inner = match &ot {
                    Type::Generic(n, args) if n == "Option" && args.len() == 1 => args[0].clone(),
                    _ => return type_err!("?. requiert Option<T>, trouvé {}", ot),
                };
                // Les champs sont privés — vérifier l'accès (jamais via this pour ?.)
                self.check_field_access_visibility(&inner, field, false);
                let ft = self.find_field_type(&inner, field)
                    .ok_or_else(|| TypeError(format!("Champ inconnu '{}'", field)))?;
                Ok(Type::Generic("Option".to_string(), vec![ft]))
            }

            Expr::SafeMethodCall { object, method, args } => {
                let ot = self.infer_expr(object, env)?;
                let ot = self.resolve(&ot);
                let inner = match &ot {
                    Type::Generic(n, ta) if n == "Option" && ta.len() == 1 => ta[0].clone(),
                    _ => return type_err!("?. requiert Option<T>, trouvé {}", ot),
                };
                let (ptys, rty, subst) = self.resolve_method(&inner, method)?;
                if args.len() != ptys.len() {
                    return type_err!("{}() : {} arg(s) attendus, {} fournis",
                        method, ptys.len(), args.len());
                }
                for (arg, pt) in args.iter().zip(ptys.iter()) {
                    if let Some(at) = self.infer(arg, env) {
                        let expected = substitute(pt, &subst);
                        if !self.is_compatible(&at, &expected) {
                            self.err(format!("Arg de {}() : type incompatible {} ≠ {}", method, at, expected));
                        }
                    }
                }
                Ok(Type::Generic("Option".to_string(), vec![substitute(&rty, &subst)]))
            }

            // ── Null coalescing  expr ?? default ─────────────────────────────

            Expr::NullCoalesce { expr, default } => {
                let et = self.infer_expr(expr, env)?;
                let et = self.resolve(&et);
                let dt = self.infer_expr(default, env)?;
                match &et {
                    Type::Generic(n, args) if n == "Option" && args.len() == 1 => {
                        if !self.is_compatible(&dt, &args[0]) {
                            self.err(format!(
                                "?? : valeur par défaut {} incompatible avec Option<{}>",
                                dt, args[0]));
                        }
                        Ok(args[0].clone())
                    }
                    _ => type_err!("?? requiert Option<T> à gauche, trouvé {}", et),
                }
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
                                    self.err(format!("Arg lambda : type incompatible {} ≠ {}", at, pt));
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

            // ── Tableau littéral : new T[]{a, b, ...} ────────────────────────
            Expr::ArrayLit { elem_type, elements } => {
                for e in elements {
                    if let Ok(et) = self.infer_expr(e, env) {
                        if !self.is_compatible(&et, elem_type) {
                            self.err(format!(
                                "Élément de tableau : type incompatible {} ≠ {}", et, elem_type));
                        }
                    }
                }
                Ok(Type::Array(Box::new(elem_type.clone())))
            }

            // ── Nouveau tableau : new T[n] ou new T[n](fill) ─────────────────
            Expr::ArrayNew { elem_type, size, fill } => {
                if let Ok(st) = self.infer_expr(size, env) {
                    if st != Type::Int {
                        self.err(format!("Taille de tableau doit être int, trouvé {}", st));
                    }
                }
                if let Some(f) = fill {
                    if let Ok(ft) = self.infer_expr(f, env) {
                        if !self.is_compatible(&ft, elem_type) {
                            self.err(format!(
                                "Valeur initiale de type {} incompatible avec le tableau {}[]",
                                ft, elem_type
                            ));
                        }
                    }
                }
                Ok(Type::Array(Box::new(elem_type.clone())))
            }

            // ── Accès indexé : arr[i] — retourne Option<T> ───────────────────
            Expr::Index { object, index } => {
                let ot = self.infer_expr(object, env)?;
                let ot = self.resolve(&ot);
                let it = self.infer_expr(index, env)?;
                if it != Type::Int {
                    self.err(format!("Index doit être int, trouvé {}", it));
                }
                match ot {
                    Type::Array(elem) => Ok(Type::Generic(
                        "Option".to_string(),
                        vec![*elem],
                    )),
                    _ => type_err!("Accès index sur non-tableau : {}", ot),
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
        // Object est le supertype universel : tout est compatible avec Object
        if let Type::UserDefined(n) = &expected { if n == "Object" { return true; } }
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
                self.is_subclass(an, en) && aa.len() == ea.len()
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
        if sup == "Object" { return true; }
        if let Some(c) = self.classes.get(sub) {
            if let Some(p) = &c.parent { if self.is_subclass(p, sup) { return true; } }
            for iface in &c.implements {
                if iface == sup { return true; }
            }
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
            Type::Array(inner)     => ("Array".to_string(), vec![*inner.clone()]),
            Type::Str              => ("String".to_string(),    vec![]),
            Type::Char             => ("Character".to_string(), vec![]),
            Type::Bool             => ("Boolean".to_string(),   vec![]),
            Type::Int              => ("Integer".to_string(),   vec![]),
            Type::Float            => ("Float".to_string(),     vec![]),
            Type::Double           => ("Double".to_string(),    vec![]),
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
                let subst: Vec<(String, Type)> = ed.type_params.iter()
                    .zip(type_args.iter())
                    .map(|(p, t)| (p.clone(), t.clone()))
                    .collect();
                return Ok((
                    m.params.iter().map(|p| substitute(&p.ty, &subst)).collect(),
                    substitute(&m.return_type, &subst),
                    subst,
                ));
            }
        }
        if let Some(iface) = self.interfaces.get(&cn) {
            if let Some(sig) = iface.methods.iter().find(|m| m.name == method) {
                let subst: Vec<(String, Type)> = iface.type_params.iter()
                    .zip(type_args.iter())
                    .map(|(p, t)| (p.clone(), t.clone()))
                    .collect();
                return Ok((
                    sig.params.iter().map(|p| substitute(&p.ty, &subst)).collect(),
                    substitute(&sig.return_type, &subst),
                    subst,
                ));
            }
        }
        type_err!("Méthode '{}' inconnue dans '{}'", method, cn)
    }

    // ── Lookup ────────────────────────────────────────────────────────────────

    fn find_method_def<'a>(&'a self, cn: &str, mn: &str) -> Option<&'a Method> {
        let c = self.classes.get(cn)?;
        if let Some(m) = c.methods.iter().find(|m| m.name == mn) { return Some(m); }
        if let Some(p) = &c.parent { return self.find_method_def(p, mn); }
        if cn != "Object" { return self.find_method_def("Object", mn); }
        None
    }

    // ── Vérifications de visibilité ───────────────────────────────────────────

    /// Retourne la visibilité ET le nom de la classe qui déclare la méthode.
    /// Ne clone que `Visibility` (enum cheap) et `String` (nom de classe),
    /// jamais le corps de la méthode — évite les stack overflows sur les
    /// méthodes complexes de la stdlib.
    fn find_method_visibility(&self, cn: &str, mn: &str) -> Option<(Visibility, String)> {
        let c = self.classes.get(cn)?;
        if let Some(m) = c.methods.iter().find(|m| m.name == mn) {
            return Some((m.visibility.clone(), cn.to_string()));
        }
        if let Some(p) = &c.parent { return self.find_method_visibility(p, mn); }
        if cn != "Object"          { return self.find_method_visibility("Object", mn); }
        None
    }

    /// Vérifie que l'appel de méthode respecte la visibilité déclarée.
    /// - `Public`    : toujours autorisé.
    /// - `Protected` : autorisé depuis la classe déclarante ou une sous-classe.
    /// - `Private`   : autorisé depuis la classe déclarante uniquement.
    fn check_method_visibility(&mut self, obj_ty: &Type, method_name: &str) {
        let cn = match obj_ty {
            Type::UserDefined(n) | Type::Generic(n, _) => n.clone(),
            _ => return, // primitifs / enums → pas de contrôle de visibilité de classe
        };
        // On ne contrôle que les classes (pas les interfaces ni les enums)
        if !self.classes.contains_key(&cn) { return; }

        let Some((visibility, declaring)) = self.find_method_visibility(&cn, method_name) else { return };

        match visibility {
            Visibility::Public => {} // toujours OK
            Visibility::Private => {
                if self.current_class.as_deref() != Some(&declaring) {
                    self.err(format!(
                        "Méthode privée '{}' de '{}' : inaccessible depuis ce contexte",
                        method_name, declaring));
                }
            }
            Visibility::Protected => {
                let allowed = match &self.current_class {
                    Some(cc) => cc == &declaring || self.is_subclass(cc, &declaring),
                    None     => false,
                };
                if !allowed {
                    self.err(format!(
                        "Méthode protégée '{}' de '{}' : accessible uniquement depuis la classe ou ses sous-classes",
                        method_name, declaring));
                }
            }
        }
    }

    /// Vérifie la règle de privé-par-classe : accès autorisé depuis `this` ou
    /// depuis une méthode de la même classe (ou sous-classe) — pattern Java/C++.
    fn check_field_access_visibility(&mut self, obj_ty: &Type, field: &str, via_this: bool) {
        let cn = match obj_ty {
            Type::UserDefined(n) | Type::Generic(n, _) => n.clone(),
            _ => return,
        };
        if !self.classes.contains_key(&cn) { return; }
        if via_this { return; }
        let same_class = match &self.current_class {
            Some(cc) => cc == &cn || self.is_subclass(cc, &cn),
            None => false,
        };
        if !same_class {
            self.err(format!(
                "Champ '{}' de '{}' est privé : accessible uniquement depuis la classe",
                field, cn));
        }
    }

    // ── Vérifications d'immutabilité ──────────────────────────────────────────

    /// Retourne true si la méthode `method` sur le type `obj_ty` est déclarée `mutable`.
    /// Vérifie dans cet ordre : classe → interface → enum. Builtins → false par défaut.
    fn method_is_mutable(&self, obj_ty: &Type, method: &str) -> bool {
        let cn = match obj_ty {
            Type::UserDefined(n) | Type::Generic(n, _) => n.as_str(),
            Type::Str    => "String",
            Type::Int    => "Integer",
            Type::Bool   => "Boolean",
            Type::Char   => "Character",
            Type::Float  => "Float",
            Type::Double => "Double",
            _ => return false,
        };
        // Classe
        if let Some(m) = self.find_method_def(cn, method) {
            return m.is_mutable;
        }
        // Interface
        if let Some(iface) = self.interfaces.get(cn) {
            if let Some(sig) = iface.methods.iter().find(|s| s.name == method) {
                return sig.is_mutable;
            }
        }
        // Enum (jamais mutable)
        false
    }

    /// Retourne le qualificateur d'une expression.
    /// Pour Phase 1 : seuls les Ident simples ont un qualificateur trackable.
    /// `this` dans une méthode non-mutable est traité comme `readonly`.
    /// Retourne true si le type est un type valeur (primitif).
    /// Les types valeur ne propagent pas les qualificateurs :
    /// appeler `.size()` sur un `readonly List` retourne un `int` mutable.
    fn is_value_type(ty: &Type) -> bool {
        matches!(ty, Type::Int | Type::Bool | Type::Str | Type::Char
                   | Type::Float | Type::Double | Type::Void
                   | Type::Fn | Type::FnType(_, _))
    }

    /// Retourne le qualificateur effectif d'une expression.
    ///
    /// Phase 3 — propagation transitive :
    /// le qualificateur du récepteur se propage à travers les appels de méthodes
    /// enchaînés. Les faux positifs sont impossibles en pratique car :
    /// - les types primitifs n'ont pas de méthodes mutables
    /// - les variables déclarées sans qualificateur sont Mutable dans l'env
    ///
    /// Note : on ne relance pas `infer` ici pour éviter la récursion mutuelle
    /// trop profonde sur les corps de méthodes complexes de la stdlib.
    fn qualifier_of_expr(&mut self, expr: &Expr, env: &TypeEnv) -> Qualifier {
        match expr {
            Expr::Ident(name) if name == "this" => {
                if self.current_method_mutable { Qualifier::Mutable } else { Qualifier::Readonly }
            }
            Expr::Ident(name) => env.get_qualifier(name),

            // Phase 3 : propager le qualificateur du récepteur à travers les chaînes
            Expr::MethodCall { object, .. } => self.qualifier_of_expr(object, env),

            _ => Qualifier::Mutable,
        }
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

    /// Retourne le type primitif correspondant à une classe wrapper, ou None.
    /// Permet à `this` d'avoir le bon type primitif dans les méthodes de String,
    /// Integer, Boolean, Character, Float, Double.
    fn primitive_type_for_class(class_name: &str) -> Option<Type> {
        match class_name {
            "String"    => Some(Type::Str),
            "Integer"   => Some(Type::Int),
            "Boolean"   => Some(Type::Bool),
            "Character" => Some(Type::Char),
            "Float"     => Some(Type::Float),
            "Double"    => Some(Type::Double),
            _           => None,
        }
    }

    /// Vérifie que le type `ty` est déclaré `mut` (ou est un enum / un primitif),
    /// ce qui est requis quand le qualificateur est `readonly` ou `immutable`.
    fn check_type_is_mut(&mut self, ty: &Type, var_name: &str, qualifier: &Qualifier) {
        let cn = match ty {
            // Les primitifs sont des types valeur — toujours OK
            Type::Int | Type::Bool | Type::Str | Type::Char
            | Type::Float | Type::Double | Type::Void => return,
            // Arrays et lambdas → on laisse passer pour l'instant
            Type::Array(_) | Type::Fn | Type::FnType(_, _) => return,
            Type::UserDefined(n) | Type::Generic(n, _) => n.clone(),
        };
        // Les enums sont toujours `mut` implicitement
        if self.enums.contains_key(&cn) { return; }
        // Vérifier dans les classes
        if let Some(c) = self.classes.get(&cn) {
            if !c.is_mut {
                self.err(format!(
                    "Variable '{}' ({}) : la classe '{}' doit être déclarée `mut` \
                     pour être utilisée avec le qualificateur `{}`",
                    var_name, qualifier, cn,
                    match qualifier { Qualifier::Readonly => "readonly",
                                      Qualifier::Immutable => "immutable",
                                      Qualifier::Mutable => "" }
                ));
            }
            return;
        }
        // Vérifier dans les interfaces
        if let Some(i) = self.interfaces.get(&cn) {
            if !i.is_mut {
                self.err(format!(
                    "Variable '{}' ({}) : l'interface '{}' doit être déclarée `mut` \
                     pour être utilisée avec le qualificateur `{}`",
                    var_name, qualifier, cn,
                    match qualifier { Qualifier::Readonly => "readonly",
                                      Qualifier::Immutable => "immutable",
                                      Qualifier::Mutable => "" }
                ));
            }
        }
        // Type inconnu → on laisse passer (erreur déjà remontée ailleurs)
    }

    // ── Vérification des contraintes de type params ────────────────────────────

    /// Vérifie que les arguments de type satisfont les contraintes déclarées sur
    /// les paramètres de type du générique `generic_name`.
    ///
    /// Ex. : `mut class Map<immutable K, V>` → K doit être un type `mut`
    ///        quand on écrit `Map<Point, int>`, Point est vérifié.
    fn check_generic_type_constraints(&mut self, generic_name: &str, type_args: &[Type]) {
        // Récupérer les noms et les contraintes du générique
        let (param_names, constraints): (Vec<String>, Vec<(String, Qualifier)>) = {
            if let Some(c) = self.classes.get(generic_name) {
                (c.type_params.clone(), c.type_param_constraints.clone())
            } else if let Some(i) = self.interfaces.get(generic_name) {
                (i.type_params.clone(), i.type_param_constraints.clone())
            } else if let Some(e) = self.enums.get(generic_name) {
                (e.type_params.clone(), e.type_param_constraints.clone())
            } else {
                return; // type inconnu, déjà traité ailleurs
            }
        };

        if constraints.is_empty() { return; }

        for (param_name, type_arg) in param_names.iter().zip(type_args.iter()) {
            // Y a-t-il une contrainte sur ce paramètre ?
            let constraint = constraints.iter()
                .find(|(n, _)| n == param_name)
                .map(|(_, q)| q.clone());
            let Some(constraint) = constraint else { continue };
            if constraint == Qualifier::Mutable { continue; }

            // L'argument de type doit être `mut` (classe ou interface auditée)
            let is_ok = match type_arg {
                // Types valeur — toujours acceptés
                Type::Int | Type::Bool | Type::Str | Type::Char
                | Type::Float | Type::Double => true,
                // Paramètre de type générique en cours de résolution — on fait confiance
                Type::UserDefined(n) if self.type_params.contains(n.as_str()) => true,
                Type::UserDefined(n) | Type::Generic(n, _) => {
                    self.classes   .get(n).map(|c| c.is_mut).unwrap_or(false)
                    || self.interfaces.get(n).map(|i| i.is_mut).unwrap_or(false)
                    || self.enums  .contains_key(n.as_str()) // enums sont mut implicitement
                }
                _ => true, // autres types → permissif
            };

            if !is_ok {
                let qual_str = match constraint {
                    Qualifier::Immutable => "immutable",
                    Qualifier::Readonly  => "readonly",
                    Qualifier::Mutable   => "",
                };
                self.err(format!(
                    "Argument de type '{}' pour le paramètre '{}' de '{}' : \
                     la contrainte `{}` requiert une classe déclarée `mut`",
                    type_arg, param_name, generic_name, qual_str
                ));
            }
        }
    }

    /// Retourne le type de `this` dans le contexte courant :
    /// type primitif si c'est une classe wrapper, type UserDefined sinon.
    fn this_type(&self) -> Type {
        if let Some(cn) = &self.current_class {
            if let Some(pt) = Self::primitive_type_for_class(cn) {
                return pt;
            }
            return Type::UserDefined(cn.clone());
        }
        if let Some(en) = &self.current_enum {
            return Type::UserDefined(en.clone());
        }
        Type::Void
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
    let full = format!("{}\n{}", crate::STDLIB, src);
    let program = crate::parser::program_parser()
        .parse(full.as_str())
        .map_err(|e| e.iter().map(|x| x.to_string()).collect::<Vec<_>>())?;
    let errors = TypeChecker::new(&program).check(&program);
    if errors.is_empty() { Ok(()) }
    else { Err(errors.iter().map(|e| e.0.clone()).collect()) }
}
