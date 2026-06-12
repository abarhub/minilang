// ─────────────────────────────────────────────────────────────────────────────
//  Configuration de projet — minilang.toml
//
//  Fichier optionnel découvert en remontant depuis le répertoire du fichier
//  source (ou le répertoire courant). Absent → configuration par défaut,
//  comportement inchangé. Priorité : CLI > minilang.toml > défauts.
//
//  ```toml
//  [project]
//  name = "mon-appli"
//  main = "src/app.mini"     # point d'entrée si aucun fichier passé en argument
//
//  [di]
//  modules = ["ProdModule"]  # modules de binding actifs (défaut : tous)
//
//  [runtime]
//  log = "info"              # niveau de log par défaut (RUST_LOG prioritaire)
//  ```
// ─────────────────────────────────────────────────────────────────────────────

use std::path::{Path, PathBuf};
use serde::Deserialize;
use crate::ast::Program;

/// Nom du fichier de configuration cherché à la racine du projet.
pub const CONFIG_FILE: &str = "minilang.toml";

// `deny_unknown_fields` : une section ou une clé inconnue (typo) est une
// erreur — cohérent avec la philosophie de détection précoce du langage.

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    #[serde(default)]
    pub project: ProjectSection,
    #[serde(default)]
    pub di:      DiSection,
    #[serde(default)]
    pub runtime: RuntimeSection,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectSection {
    /// Nom du projet — affiché dans les logs.
    pub name: Option<String>,
    /// Point d'entrée, relatif au répertoire du minilang.toml.
    /// Utilisé quand aucun fichier n'est passé en argument.
    pub main: Option<String>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DiSection {
    /// Modules de binding actifs. `None` = tous les modules du programme.
    /// Une liste vide désactive tous les modules.
    pub modules: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSection {
    /// Niveau de log par défaut (`error`, `warn`, `info`, `debug`, `trace`).
    /// La variable d'environnement RUST_LOG reste prioritaire.
    pub log: Option<String>,
}

impl ProjectConfig {
    /// Parse le contenu d'un minilang.toml.
    pub fn parse(content: &str) -> Result<ProjectConfig, String> {
        toml::from_str(content).map_err(|e| e.to_string())
    }
}

/// Cherche un `minilang.toml` dans `start_dir` puis dans ses répertoires
/// parents. Retourne le chemin du premier trouvé.
pub fn find_config(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = Some(start_dir);
    while let Some(d) = dir {
        let candidate = d.join(CONFIG_FILE);
        if candidate.is_file() { return Some(candidate); }
        dir = d.parent();
    }
    None
}

/// Charge la configuration du projet en partant de `start_dir`.
/// - Aucun fichier trouvé → `Ok(None)` (configuration par défaut).
/// - Fichier trouvé mais illisible ou invalide → `Err` (erreur fatale :
///   une configuration présente mais cassée ne doit pas être ignorée).
pub fn load(start_dir: &Path) -> Result<Option<(ProjectConfig, PathBuf)>, String> {
    let Some(path) = find_config(start_dir) else { return Ok(None) };
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("{} : lecture impossible : {}", path.display(), e))?;
    let config = ProjectConfig::parse(&content)
        .map_err(|e| format!("{} : {}", path.display(), e))?;
    Ok(Some((config, path)))
}

/// Restreint les modules de binding du programme à la liste `active`
/// (sélection de profil DI). Chaque nom doit correspondre à un module
/// déclaré dans le programme — sinon erreur, détectée avant le typecheck.
pub fn select_modules(program: &mut Program, active: &[String]) -> Result<(), String> {
    for name in active {
        if !program.modules.iter().any(|m| &m.name == name) {
            let mut known: Vec<&str> = program.modules.iter()
                .map(|m| m.name.as_str()).collect();
            known.sort();
            return Err(format!(
                "Module DI inconnu '{}' dans [di] modules (modules déclarés : {})",
                name,
                if known.is_empty() { "aucun".to_string() } else { known.join(", ") }
            ));
        }
    }
    program.modules.retain(|m| active.iter().any(|a| a == &m.name));
    Ok(())
}
