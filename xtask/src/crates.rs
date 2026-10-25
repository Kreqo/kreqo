// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `crates` step: every crate in the workspace belongs to a project (T2),
//! carries its project's license (LEGAL-01) and inherits the workspace lints
//! (QA-01), and every Rust file passes the attribute checks (QA-01, SEC-01).

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use cargo_metadata::{MetadataCommand, Package};
use serde::Deserialize;

use crate::{attributes, git};

/// A project's metadata, from the `project.toml` in its folder (T2).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    kind: Kind,
    stage: Stage,
    license: License,
}

/// What a project is; rules apply by kind.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Kind {
    Library,
    Tool,
    App,
    Server,
    Website,
}

/// A project's stage (LIFE-01).
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Stage {
    Incubating,
    InReview,
    Stable,
    Unmaintained,
    Archived,
}

/// The license groups (LEGAL-01).
#[derive(Clone, Copy, Deserialize)]
enum License {
    #[serde(rename = "MIT OR Apache-2.0")]
    MitOrApache,
    #[serde(rename = "GPL-3.0-or-later")]
    Gpl,
    #[serde(rename = "AGPL-3.0-or-later")]
    Agpl,
}

impl License {
    fn spdx(self) -> &'static str {
        match self {
            Self::MitOrApache => "MIT OR Apache-2.0",
            Self::Gpl => "GPL-3.0-or-later",
            Self::Agpl => "AGPL-3.0-or-later",
        }
    }
}

impl fmt::Display for Project {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            Kind::Library => "library",
            Kind::Tool => "tool",
            Kind::App => "app",
            Kind::Server => "server",
            Kind::Website => "website",
        };
        let stage = match self.stage {
            Stage::Incubating => "incubating",
            Stage::InReview => "in review",
            Stage::Stable => "stable",
            Stage::Unmaintained => "unmaintained",
            Stage::Archived => "archived",
        };
        write!(f, "{kind}, {stage}, {}", self.license.spdx())
    }
}

/// Checks every crate in the workspace and reports each problem.
pub fn check(root: &Path) -> Result<()> {
    let metadata = MetadataCommand::new()
        .current_dir(root)
        .no_deps()
        .other_options(vec!["--locked".to_owned()])
        .exec()
        .context("couldn't read the workspace's metadata")?;
    let workspace_root = metadata.workspace_root.as_std_path();
    let mut problems = Vec::new();
    let mut manifests = BTreeSet::from([workspace_root.join("Cargo.toml")]);
    let mut crate_roots = BTreeSet::new();
    for package in metadata.workspace_packages() {
        problems.extend(check_package(workspace_root, package)?);
        manifests.insert(package.manifest_path.clone().into_std_path_buf());
        crate_roots.extend(
            package
                .targets
                .iter()
                .map(|target| target.src_path.clone().into_std_path_buf()),
        );
    }
    // A crate outside the members would escape every check, so until nested
    // workspaces are supported (QA-01's copy of the lint table), none may
    // exist.
    for manifest in git::files(workspace_root, &[":(glob)**/Cargo.toml"])? {
        if !manifests.contains(&workspace_root.join(&manifest)) {
            problems.push(format!(
                "{}: a crate outside the workspace's members, which no check covers; add it to `members` in the root Cargo.toml (QA-01, T2)",
                manifest.display()
            ));
        }
    }
    if workspace_root.join("project.toml").exists() {
        problems.push(
            "project.toml: a project at the repository root would claim every crate (T2)"
                .to_owned(),
        );
    }
    problems.extend(attributes::check(workspace_root, &crate_roots)?);
    if problems.is_empty() {
        return Ok(());
    }
    for problem in &problems {
        eprintln!("{problem}");
    }
    bail!("{} problem(s) with the workspace's crates", problems.len())
}

/// Checks one crate and returns its problems.
fn check_package(workspace_root: &Path, package: &Package) -> Result<Vec<String>> {
    let name = &package.name;
    let manifest = package.manifest_path.as_std_path();
    let mut problems = Vec::new();
    match find_project(workspace_root, manifest) {
        Ok(Some((path, project))) => {
            let path = path.strip_prefix(workspace_root).unwrap_or(&path);
            println!("{name}: {project} ({})", path.display());
            let expected = project.license.spdx();
            match package.license.as_deref() {
                Some(license) if license == expected => {}
                Some(license) => problems.push(format!(
                    "{name}: its license is `{license}`, but its project's group is `{expected}` (LEGAL-01)"
                )),
                None => problems.push(format!(
                    "{name}: no `license` field; its project's group is `{expected}` (LEGAL-01)"
                )),
            }
        }
        Ok(None) => problems.push(format!(
            "{name}: no project.toml in its folder or above it (T2)"
        )),
        Err(error) => problems.push(format!("{name}: {error:#}")),
    }
    // A read error becomes a problem too, so that the step still reports
    // everything else it found.
    let inherits = fs::read_to_string(manifest)
        .map_err(anyhow::Error::from)
        .and_then(|text| inherits_workspace_lints(&text));
    match inherits {
        Ok(true) => {}
        Ok(false) => problems.push(format!(
            "{name}: its Cargo.toml lacks `[lints] workspace = true` (QA-01)"
        )),
        Err(error) => problems.push(format!(
            "{name}: couldn't read {}: {error:#}",
            manifest.display()
        )),
    }
    Ok(problems)
}

/// The project a crate belongs to: the nearest `project.toml` from the
/// crate's folder up to, but not including, the workspace root.
fn find_project(workspace_root: &Path, manifest: &Path) -> Result<Option<(PathBuf, Project)>> {
    let Some(crate_folder) = manifest.parent() else {
        return Ok(None);
    };
    for folder in crate_folder
        .ancestors()
        .take_while(|folder| folder.starts_with(workspace_root) && *folder != workspace_root)
    {
        let path = folder.join("project.toml");
        if path.is_file() {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("couldn't read {}", path.display()))?;
            let project =
                toml::from_str(&text).with_context(|| format!("{} is invalid", path.display()))?;
            return Ok(Some((path, project)));
        }
    }
    Ok(None)
}

/// Whether a crate's manifest inherits the workspace lints.
fn inherits_workspace_lints(manifest: &str) -> Result<bool> {
    let manifest: toml::Table = manifest.parse()?;
    Ok(manifest
        .get("lints")
        .and_then(|lints| lints.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str =
        "kind = \"tool\"\nstage = \"incubating\"\nlicense = \"MIT OR Apache-2.0\"\n";

    #[test]
    fn reads_a_project() -> Result<()> {
        let project: Project = toml::from_str(VALID)?;
        assert_eq!(project.to_string(), "tool, incubating, MIT OR Apache-2.0");
        Ok(())
    }

    #[test]
    fn rejects_unknown_fields_and_values() {
        let unknown_field = format!("{VALID}owner = \"someone\"\n");
        assert!(toml::from_str::<Project>(&unknown_field).is_err());
        let unknown_kind = VALID.replace("tool", "plugin");
        assert!(toml::from_str::<Project>(&unknown_kind).is_err());
        let other_license = VALID.replace("MIT OR Apache-2.0", "MIT");
        assert!(toml::from_str::<Project>(&other_license).is_err());
    }

    #[test]
    fn detects_inherited_lints() -> Result<()> {
        assert!(inherits_workspace_lints(
            "[package]\nname = \"a\"\n\n[lints]\nworkspace = true\n"
        )?);
        assert!(!inherits_workspace_lints("[package]\nname = \"a\"\n")?);
        assert!(!inherits_workspace_lints(
            "[lints.rust]\nunsafe_code = \"deny\"\n"
        )?);
        Ok(())
    }
}
