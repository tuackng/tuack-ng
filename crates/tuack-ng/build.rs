use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use vergen::{BuildBuilder, CargoBuilder, Emitter, RustcBuilder, SysinfoBuilder};

/// 返回 workspace 根目录（crates/tuack-ng 的父目录的父目录），assets/vendor 都在根下
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// 把 testlib.h 复制到 `assets/checkers`，目标已存在且内容一致时跳过。
fn copy_testlib(source: PathBuf) -> io::Result<()> {
    let checkers_dir = workspace_root().join("assets/checkers");
    let testlib_dest = checkers_dir.join("testlib.h");

    println!("cargo:rerun-if-changed={}", checkers_dir.display());
    println!("cargo:rerun-if-changed={}", source.display());

    fs::create_dir_all(&checkers_dir)?;

    if !source.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("testlib.h not found at {}", source.display()),
        ));
    }

    let should_copy = if testlib_dest.exists() {
        let src_content = fs::read(&source)?;
        let dst_content = fs::read(&testlib_dest)?;
        src_content != dst_content
    } else {
        true
    };

    if should_copy {
        fs::copy(&source, &testlib_dest)?;
    }
    Ok(())
}

fn main() {
    let checkers_dir = workspace_root().join("assets/checkers");
    println!("cargo:rerun-if-changed={}", checkers_dir.display());
    #[cfg(not(feature = "nix"))]
    {
        copy_testlib(workspace_root().join("vendor/testlib/testlib.h")).unwrap();
    }
    #[cfg(feature = "nix")]
    {
        // Nix 构建从 NIX_TESTLIB_PATH 读取 testlib.h（Nix store 路径）
        let testlib_path = std::env::var("NIX_TESTLIB_PATH").unwrap();
        copy_testlib(testlib_path.into()).unwrap();
    }
    if let Ok(entries) = fs::read_dir(&checkers_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "cpp") {
                compile_cpp_if_needed(&path);
            }
        }
    }

    let build = BuildBuilder::default()
        .build_timestamp(true)
        .build()
        .unwrap();
    let cargo = CargoBuilder::default()
        .features(true)
        .debug(true)
        .opt_level(true)
        .target_triple(true)
        .dependencies(true)
        .build()
        .unwrap();
    let rustc = RustcBuilder::default()
        .semver(true)
        .channel(true)
        .build()
        .unwrap();
    let si = SysinfoBuilder::default().os_version(true).build().unwrap();

    Emitter::default()
        .add_instructions(&build)
        .unwrap()
        .add_instructions(&cargo)
        .unwrap()
        .add_instructions(&rustc)
        .unwrap()
        .add_instructions(&si)
        .unwrap()
        .quiet()
        .emit()
        .unwrap();
}

/// 用 g++ 重新编译源码较新（或产物缺失）的 checker。
fn compile_cpp_if_needed(cpp_file: &Path) {
    let exe_name = cpp_file.with_extension(env::consts::EXE_EXTENSION);
    let exe_name = exe_name.file_name().unwrap().to_string_lossy();
    let exe_path = cpp_file.parent().unwrap().join(exe_name.to_string());

    let need_compile = if exe_path.exists() {
        let src_modified = fs::metadata(cpp_file)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let exe_modified = fs::metadata(&exe_path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        src_modified > exe_modified
    } else {
        true
    };

    if need_compile {
        println!("cargo:warning=Compiling: {}", cpp_file.display());

        let mut cmd = Command::new("g++");
        cmd.current_dir(cpp_file.parent().unwrap())
            .arg("-std=c++17")
            .arg("-O2")
            .arg(cpp_file.file_name().unwrap())
            .arg("-o")
            .arg(exe_name.to_string());
        #[cfg(not(feature = "nix"))]
        {
            // 默认静态链接 checker 便于分发；NixOS 无静态 glibc，nix feature 下不传该参数
            cmd.arg("-static");
        }

        let status = cmd.status();

        if let Ok(status) = status {
            if !status.success() {
                println!("cargo:warning=Failed to compile: {}", cpp_file.display());
            }
        } else {
            println!(
                "cargo:warning=Failed to execute g++ for: {}",
                cpp_file.display()
            );
        }
    }

    println!("cargo:rerun-if-changed={}", cpp_file.display());
}
