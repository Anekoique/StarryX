// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0 OR MulanPSL-2.0

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const DEFAULT_REPOSITORY: &str = "https://github.com/asterinas/linux_vdso.git";
const DEFAULT_REVISION: &str = "74898350d406d6cd8988531ad737380a8e2cdbf4";

fn main() {
    println!("cargo:rerun-if-env-changed=XVDSO_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=XVDSO_REPOSITORY");
    println!("cargo:rerun-if-env-changed=XVDSO_REVISION");

    let image_name = image_name(&required_env("CARGO_CFG_TARGET_ARCH"));
    let source_dir = match env::var_os("XVDSO_SOURCE_DIR") {
        Some(path) => PathBuf::from(path),
        None => {
            let repository =
                env::var("XVDSO_REPOSITORY").unwrap_or_else(|_| DEFAULT_REPOSITORY.into());
            let revision = env::var("XVDSO_REVISION").unwrap_or_else(|_| DEFAULT_REVISION.into());
            checkout_provider(Path::new(&required_env("OUT_DIR")), &repository, &revision)
        }
    };

    let image_path = source_dir.join(image_name);
    if !image_path.is_file() {
        panic!(
            "xvdso provider `{}` does not contain `{image_name}` for the selected architecture",
            source_dir.display()
        );
    }

    println!("cargo:rerun-if-changed={}", image_path.display());
    println!("cargo:rustc-env=XVDSO_IMAGE_PATH={}", image_path.display());
}

fn image_name(arch: &str) -> &'static str {
    match arch {
        "riscv64" => "vdso_riscv64.so",
        "loongarch64" => "vdso_loongarch64.so",
        other => panic!("xvdso does not support target architecture `{other}`"),
    }
}

fn checkout_provider(out_dir: &Path, repository: &str, revision: &str) -> PathBuf {
    let checkout = out_dir.join("linux_vdso");
    if checkout_matches(&checkout, repository, revision) {
        return checkout;
    }

    if checkout.exists() {
        fs::remove_dir_all(&checkout).unwrap_or_else(|error| {
            panic!(
                "failed to remove stale xvdso provider `{}`: {error}",
                checkout.display()
            )
        });
    }

    println!("cargo:warning=xvdso: cloning pinned provider {repository}@{revision}");
    run_git(
        Command::new("git")
            .arg("clone")
            .arg("--filter=blob:none")
            .arg("--no-checkout")
            .arg(repository)
            .arg(&checkout),
        "clone the xvdso provider",
    );
    run_git(
        Command::new("git")
            .arg("-C")
            .arg(&checkout)
            .arg("checkout")
            .arg("--detach")
            .arg(revision),
        "check out the pinned xvdso revision",
    );

    if !checkout_matches(&checkout, repository, revision) {
        panic!(
            "xvdso provider checkout `{}` does not match {repository}@{revision}",
            checkout.display()
        );
    }
    checkout
}

fn checkout_matches(checkout: &Path, repository: &str, revision: &str) -> bool {
    if !checkout.join(".git").is_dir() {
        return false;
    }

    let Some(origin) = git_stdout(Command::new("git").arg("-C").arg(checkout).args([
        "config",
        "--get",
        "remote.origin.url",
    ])) else {
        return false;
    };
    if origin != repository {
        return false;
    }

    let Some(head) = git_stdout(
        Command::new("git")
            .arg("-C")
            .arg(checkout)
            .args(["rev-parse", "HEAD"]),
    ) else {
        return false;
    };
    let revision_commit = format!("{revision}^{{commit}}");
    let Some(expected) = git_stdout(
        Command::new("git")
            .arg("-C")
            .arg(checkout)
            .args(["rev-parse", &revision_commit]),
    ) else {
        return false;
    };
    head == expected
}

fn run_git(command: &mut Command, action: &str) {
    command.env("GIT_TERMINAL_PROMPT", "0");
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("failed to {action}: {error}"));
    if !output.status.success() {
        panic!(
            "failed to {action}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
}

fn git_stdout(command: &mut Command) -> Option<String> {
    command.env("GIT_TERMINAL_PROMPT", "0");
    successful_output(command.output().ok()?)
}

fn successful_output(output: Output) -> Option<String> {
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("required build environment `{name}` is unset"))
}
