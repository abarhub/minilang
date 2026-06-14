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

use crate::ast::Program;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    pub sources: SourcesSection,
    #[serde(default)]
    pub di: DiSection,
    #[serde(default)]
    pub tests: TestsSection,
    #[serde(default)]
    pub files: FilesSection,
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
pub struct SourcesSection {
    /// Fichiers source additionnels (bibliothèque du projet, sans `main`),
    /// relatifs au répertoire du minilang.toml. Ils sont préfixés au fichier
    /// exécuté — en mode run comme en mode test — comme l'est la stdlib.
    pub include: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TestsSection {
    /// Répertoire des fichiers de tests, relatif au minilang.toml (défaut : "tests").
    pub dir: Option<String>,
    /// Modules de binding actifs pendant les tests (profil DI de test).
    /// Défaut : la valeur de [di] modules, sinon tous les modules.
    pub modules: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DiSection {
    /// Modules de binding actifs. `None` = tous les modules du programme.
    /// Une liste vide désactive tous les modules.
    pub modules: Option<Vec<String>>,
}

/// Mode d'accès d'une racine fichiers configurée.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum FileMode {
    #[default]
    Read, // "read"        — lecture seule (défaut, sûr-par-défaut)
    ReadWrite, // "read-write"  — lecture/écriture
}

/// Une racine fichiers nommée : un chemin + son mode d'accès.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RootConfig {
    /// Chemin du répertoire, relatif au minilang.toml (ou absolu).
    pub path: String,
    /// Mode d'accès (défaut : `read`).
    #[serde(default)]
    pub mode: FileMode,
}

/// Politique de nettoyage des répertoires temporaires créés par
/// `FileSystem.tempDir()`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TempCleanup {
    /// Ne rien faire : pas de marqueur, pas de suppression.
    None,
    /// Poser un marqueur `.minilang-temp` à la création (un nettoyeur externe
    /// supprimera les répertoires marqués selon leur âge). Défaut.
    #[default]
    Mark,
    /// Supprimer les répertoires temporaires en fin de programme (best-effort) ;
    /// le marqueur reste posé comme filet de sécurité en cas d'arrêt anormal.
    Delete,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FilesSection {
    /// Racines nommées octroyées au programme : nom → { path, mode }.
    /// Une capacité de répertoire s'obtient via `FileSystem.root(nom)` /
    /// `rootRW(nom)`. Les répertoires doivent exister au démarrage.
    pub roots: Option<HashMap<String, RootConfig>>,
    /// Nettoyage des répertoires temporaires (`tempDir()`). Défaut : `mark`.
    #[serde(default)]
    pub temp: TempCleanup,
    /// Autorise l'accès fichiers BRUT (classe `Files`, chemins arbitraires, sans
    /// confinement). Sûr par défaut : `false` → seules les capacités confinées
    /// (`FileSystem` / racines) sont utilisables.
    #[serde(default)]
    pub unrestricted: bool,
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
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

/// Charge la configuration du projet en partant de `start_dir`.
/// - Aucun fichier trouvé → `Ok(None)` (configuration par défaut).
/// - Fichier trouvé mais illisible ou invalide → `Err` (erreur fatale :
///   une configuration présente mais cassée ne doit pas être ignorée).
pub fn load(start_dir: &Path) -> Result<Option<(ProjectConfig, PathBuf)>, String> {
    let Some(path) = find_config(start_dir) else {
        return Ok(None);
    };
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("{} : lecture impossible : {}", path.display(), e))?;
    let config =
        ProjectConfig::parse(&content).map_err(|e| format!("{} : {}", path.display(), e))?;
    Ok(Some((config, path)))
}

/// Résout et valide les racines fichiers configurées (`[files.roots]`).
/// - Chemins relatifs résolus par rapport au répertoire du minilang.toml.
/// - Chaque répertoire doit **exister** (sinon erreur fatale) ; le chemin est
///   canonicalisé (absolu, liens résolus) — c'est la racine de confinement.
/// Retourne une map `nom → (chemin absolu, writable)`.
pub fn resolve_roots(
    files: &FilesSection,
    cfg_dir: Option<&Path>,
) -> Result<HashMap<String, (String, bool)>, String> {
    let mut out = HashMap::new();
    let Some(roots) = &files.roots else {
        return Ok(out);
    };
    let base = cfg_dir.unwrap_or(Path::new("."));
    // Ordre déterministe pour des messages d'erreur stables.
    let mut names: Vec<&String> = roots.keys().collect();
    names.sort();
    for name in names {
        let rc = &roots[name];
        let raw = Path::new(&rc.path);
        let joined = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            base.join(raw)
        };
        let canon = std::fs::canonicalize(&joined).map_err(|e| {
            format!(
                "racine '{}' : répertoire introuvable '{}' ({})",
                name,
                joined.display(),
                e
            )
        })?;
        if !canon.is_dir() {
            return Err(format!(
                "racine '{}' : '{}' n'est pas un répertoire",
                name,
                canon.display()
            ));
        }
        let writable = rc.mode == FileMode::ReadWrite;
        out.insert(
            name.clone(),
            (canon.to_string_lossy().to_string(), writable),
        );
    }
    Ok(out)
}

/// Restreint les modules de binding du programme à la liste `active`
/// (sélection de profil DI). Chaque nom doit correspondre à un module
/// déclaré dans le programme — sinon erreur, détectée avant le typecheck.
pub fn select_modules(program: &mut Program, active: &[String]) -> Result<(), String> {
    for name in active {
        if !program.modules.iter().any(|m| &m.name == name) {
            let mut known: Vec<&str> = program.modules.iter().map(|m| m.name.as_str()).collect();
            known.sort();
            return Err(format!(
                "Module DI inconnu '{}' dans [di] modules (modules déclarés : {})",
                name,
                if known.is_empty() {
                    "aucun".to_string()
                } else {
                    known.join(", ")
                }
            ));
        }
    }
    program
        .modules
        .retain(|m| active.iter().any(|a| a == &m.name));
    Ok(())
}
