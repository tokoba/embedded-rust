use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use cargo_metadata::{MetadataCommand, Package, Target, TargetKind};
use clap::{Parser, Subcommand};

const EMBEDDED_TARGET: &str = "thumbv7em-none-eabihf";
const HOST_TARGET: &str = "host-tuple";

#[derive(Parser, Debug)]
#[command(
  author,
  version,
  about = "Workspace task runner for embedded/host development"
)]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
  /// Run host unit tests using cargo-nextest.
  TestHost {
    /// Also run doctests with cargo test --doc.
    #[arg(long)]
    with_doctests: bool,
    /// Pass --nocapture to nextest/cargo test where supported.
    #[arg(long, short)]
    verbose: bool,
    /// Additional args passed after -- to cargo nextest run.
    #[arg(last = true)]
    args: Vec<String>,
  },
  /// Run clippy for host-testable library crates.
  ClippyHost,
  /// Build embedded binaries.
  BuildEmbedded,
  /// Run clippy for embedded binaries.
  ClippyEmbedded,
  /// Run formatter check, host lint/test, embedded lint/build.
  Ci,
  /// Print host-testable packages detected by xtask.
  ListHostPackages,
}

fn main() -> Result<()> {
  let cli = Cli::parse();
  match cli.command {
    Commands::TestHost {
      with_doctests,
      verbose,
      args,
    } => test_host(with_doctests, verbose, &args),
    Commands::ClippyHost => clippy_host(),
    Commands::BuildEmbedded => build_embedded(),
    Commands::ClippyEmbedded => clippy_embedded(),
    Commands::Ci => ci(),
    Commands::ListHostPackages => {
      let pkgs = host_testable_packages()?;
      for p in pkgs {
        println!("{}", p.name);
      }
      Ok(())
    }
  }
}

fn ci() -> Result<()> {
  run("cargo", &["fmt", "--all", "--", "--check"])?;
  clippy_host()?;
  test_host(false, false, &[])?;
  clippy_embedded()?;
  build_embedded()?;
  Ok(())
}

fn test_host(with_doctests: bool, verbose: bool, extra_args: &[String]) -> Result<()> {
  ensure_nextest_installed()?;
  let pkgs = host_testable_packages()?;
  if pkgs.is_empty() {
    println!("No host-testable packages were detected. Nothing to test.");
    return Ok(());
  }

  let mut args = vec!["nextest".into(), "run".into(), "--workspace".into()];
  add_package_args(&mut args, &pkgs);
  args.extend(["--target".into(), HOST_TARGET.into(), "--lib".into()]);
  if verbose {
    args.push("--nocapture".into());
  }
  args.extend(extra_args.iter().cloned());
  run_owned("cargo", &args)?;

  if with_doctests {
    let mut doc_args = vec!["test".into(), "--doc".into(), "--workspace".into()];
    add_package_args(&mut doc_args, &pkgs);
    doc_args.extend(["--target".into(), HOST_TARGET.into()]);
    if verbose {
      doc_args.extend(["--".into(), "--nocapture".into()]);
    }
    run_owned("cargo", &doc_args)?;
  }
  Ok(())
}

fn clippy_host() -> Result<()> {
  let pkgs = host_testable_packages()?;
  if pkgs.is_empty() {
    println!("No host-testable packages were detected. Nothing to lint.");
    return Ok(());
  }

  for pkg in pkgs {
    let args = [
      "clippy",
      "-p",
      pkg.name.as_str(),
      "--target",
      HOST_TARGET,
      "--lib",
      "--tests",
      "--",
      "-D",
      "warnings",
    ];
    run("cargo", &args)?;
  }
  Ok(())
}

fn build_embedded() -> Result<()> {
  run(
    "cargo",
    &[
      "build",
      "--workspace",
      "--exclude",
      "xtask",
      "--target",
      EMBEDDED_TARGET,
      "--bins",
    ],
  )
}

fn clippy_embedded() -> Result<()> {
  run(
    "cargo",
    &[
      "clippy",
      "--workspace",
      "--exclude",
      "xtask",
      "--target",
      EMBEDDED_TARGET,
      "--no-default-features",
      "--bins",
      "--",
      "-D",
      "warnings",
    ],
  )
}

fn ensure_nextest_installed() -> Result<()> {
  let status = Command::new("cargo")
    .args(["nextest", "--version"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .context("failed to check cargo-nextest installation")?;
  if !status.success() {
    bail!("cargo-nextest is not installed. Install it with: cargo install cargo-nextest");
  }
  Ok(())
}

fn host_testable_packages() -> Result<Vec<Package>> {
  let metadata = MetadataCommand::new().no_deps().exec()?;
  let mut packages = Vec::new();

  for pkg in metadata.packages {
    if !metadata.workspace_members.contains(&pkg.id) {
      continue;
    }
    if pkg.name == "xtask" {
      continue;
    }

    let explicit = host_testable_metadata(&pkg);
    let has_lib = pkg.targets.iter().any(is_lib_target);

    let include = match explicit {
      Some(false) => false,
      Some(true) => true,
      None => has_lib,
    };

    if include {
      packages.push(pkg);
    }
  }

  packages.sort_by(|a, b| a.name.cmp(&b.name));
  Ok(packages)
}

fn host_testable_metadata(pkg: &Package) -> Option<bool> {
  pkg.metadata.get("ci")?.get("host-testable")?.as_bool()
}

fn is_lib_target(target: &Target) -> bool {
  target.kind.iter().any(|kind| matches!(kind, TargetKind::Lib | TargetKind::RLib))
}

fn add_package_args(args: &mut Vec<String>, pkgs: &[Package]) {
  for pkg in pkgs {
    args.push("-p".into());
    args.push(pkg.name.to_string());
  }
}

fn run(program: &str, args: &[&str]) -> Result<()> {
  println!("$ {} {}", program, args.join(" "));
  let status = Command::new(program).args(args).status()?;
  if !status.success() {
    bail!("command failed: {} {}", program, args.join(" "));
  }
  Ok(())
}

fn run_owned(program: &str, args: &[String]) -> Result<()> {
  println!("$ {} {}", program, args.join(" "));
  let status = Command::new(program).args(args).status()?;
  if !status.success() {
    bail!("command failed: {} {}", program, args.join(" "));
  }
  Ok(())
}
