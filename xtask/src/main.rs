use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Context, Result};
use molecule_codegen::{Compiler, Language};

const SCHEMAS: &[&str] = &["blockchain.mol", "core.mol", "witness.mol"];

fn main() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [cmd, target] if cmd == "codegen" && target == "cobuild-types" => codegen(false),
        [cmd, target, flag] if cmd == "codegen" && target == "cobuild-types" && flag == "--check" => {
            codegen(true)
        }
        _ => bail!("usage: cargo run -p xtask -- codegen cobuild-types [--check]"),
    }
}

fn codegen(check: bool) -> Result<()> {
    let root = workspace_root()?;
    let schema_dir = root.join("crates/cobuild-types/schemas");
    let checked_in = root.join("crates/cobuild-types/src");
    let output_root = if check {
        root.join("target/xtask-codegen-check/cobuild-types/src")
    } else {
        checked_in.clone()
    };

    generate_family(&schema_dir, &output_root.join("lazy_reader"), Language::RustLazyReader)?;
    generate_family(&schema_dir, &output_root.join("entity"), Language::Rust)?;

    if check {
        compare_dirs(&checked_in.join("lazy_reader"), &output_root.join("lazy_reader"))?;
        compare_dirs(&checked_in.join("entity"), &output_root.join("entity"))?;
    }

    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask must live under workspace root")?
        .to_path_buf())
}

fn generate_family(schema_dir: &Path, out_dir: &Path, language: Language) -> Result<()> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    prune_rs_files(out_dir)?;

    for schema in SCHEMAS {
        Compiler::new()
            .generate_code(language)
            .input_schema_file(schema_dir.join(schema))
            .output_dir(out_dir)
            .run()
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("failed to generate {schema}"))?;
        run_rustfmt(&out_dir.join(schema).with_extension("rs"))?;
    }

    fs::write(out_dir.join("mod.rs"), module_file())
        .with_context(|| format!("failed to write {}", out_dir.join("mod.rs").display()))?;
    run_rustfmt(&out_dir.join("mod.rs"))?;
    Ok(())
}

fn prune_rs_files(out_dir: &Path) -> Result<()> {
    if !out_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(out_dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|ext| ext == "rs") {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn module_file() -> &'static str {
    "#![allow(dead_code)]\n#![allow(clippy::all)]\npub mod blockchain;\npub mod core;\npub mod witness;\n"
}

fn run_rustfmt(path: &Path) -> Result<()> {
    let status = Command::new("rustfmt")
        .arg("--edition")
        .arg("2021")
        .arg(path)
        .status()?;
    if !status.success() {
        bail!("rustfmt failed for {}", path.display());
    }
    Ok(())
}

fn compare_dirs(expected: &Path, actual: &Path) -> Result<()> {
    for name in ["mod.rs", "blockchain.rs", "core.rs", "witness.rs"] {
        let expected_text = fs::read_to_string(expected.join(name))
            .with_context(|| format!("missing {}", expected.join(name).display()))?;
        let actual_text = fs::read_to_string(actual.join(name))
            .with_context(|| format!("missing {}", actual.join(name).display()))?;
        if expected_text != actual_text {
            bail!("generated output differs for {}", expected.join(name).display());
        }
    }
    Ok(())
}
