use cargo_metadata::{Metadata, MetadataCommand, Package};
use clap::{crate_description, crate_name, crate_version, App, AppSettings, SubCommand};
use llvm_tools::{exe, LlvmTools};
use std::{
    env,
    fs::OpenOptions,
    path::{Path, PathBuf},
    process::Command,
};

/// Returns path to the bootloader binary.
fn build_bootloader(meta: &Metadata, kernel_image: &Path) -> PathBuf {
    let bootloader: &Package = meta
        .packages
        .iter()
        .find(|x| x.name == "bootloader")
        .expect("Missing bootloader dependency");
    let bootloader_manifest = Path::new(&bootloader.manifest_path);
    let bootloader_target = bootloader_manifest.with_file_name("x86_64-bootloader.json");
    let bootloader_triple = bootloader_target.file_stem().expect("Impossible");
    let cargo = env::var_os("CARGO").expect("Missing CARGO environment variable.");
    // Build bootloader sysroot
    let exit = Command::new(&cargo)
        .arg("sysroot")
        .arg("--target")
        .arg(&bootloader_target)
        .arg("--no-config")
        .current_dir(bootloader_manifest.parent().expect("Impossible"))
        .status()
        .expect("Failed to build bootloader sysroot");
    assert!(exit.success(), "Failed to build bootloader sysroot");
    // Build bootloader
    let mut exit = Command::new(&cargo);
    exit.arg("build")
        // Required now, for some reason.
        .env("KERNEL", kernel_image)
        .env("KERNEL_MANIFEST", meta.workspace_root.join("Cargo.toml"))
        //
        .arg("--release")
        .arg("--target")
        .arg(&bootloader_target)
        .env(
            "RUSTFLAGS",
            format!(
                "--sysroot {}",
                bootloader_manifest
                    .with_file_name("target")
                    .join("sysroot")
                    .to_str()
                    .expect("Invalid path")
            ),
        )
        .current_dir(bootloader_manifest.parent().expect("Impossible"));
    // Only include binary feature if it exists.
    // This allows supporting older `bootloader` versions.
    if bootloader.features.contains_key("binary") {
        exit.arg("--features").arg("binary");
    }
    let exit = exit.status().expect("Failed to build bootloader");
    //
    assert!(exit.success(), "Failed to build bootloader");
    bootloader_manifest
        .with_file_name("target")
        .join(bootloader_triple)
        .join("release")
        .join("bootloader")
}

/// Returns path to the kernel binary
fn build_kernel(meta: &Metadata) -> PathBuf {
    let selfa = meta
        .packages
        .iter()
        .find(|x| {
            Path::new(&x.manifest_path)
                .canonicalize()
                .expect("Impossible")
                == Path::new("Cargo.toml").canonicalize().expect("Impossible")
        })
        .expect("Couldn't find self in cargo-metadata")
        .targets
        .iter()
        .find(|x| x.kind.iter().find(|x| *x == "bin").is_some())
        .expect("Couldn't find a bin target.");
    let cargo = env::var_os("CARGO").expect("Missing CARGO environment variable.");
    let target = PathBuf::from(
        cargo_toml2::from_path::<_, cargo_toml2::CargoConfig>(".cargo/config")
            .expect("Couldn't read .cargo/config")
            .build
            .expect("Couldn't read [build]")
            .target
            .expect("Couldn't read target"),
    );
    let target_triple = target.file_stem().expect("Impossible");
    // Build sysroot, just in case.
    let exit = Command::new(&cargo)
        .arg("sysroot")
        .status()
        .expect("Failed to build kernel sysroot");
    assert!(exit.success(), "Failed to build kernel sysroot");
    // Build kernel
    let exit = Command::new(&cargo)
        .arg("build")
        .status()
        .expect("Failed to build kernel");
    assert!(exit.success(), "Failed to build kernel");
    //
    // TODO: --release support
    PathBuf::from(&meta.target_directory)
        .join(target_triple)
        .join("debug")
        .join(&selfa.name)
}

/// Creates the final image by combining the bootloader and the kernel.
fn create_image<T: AsRef<Path>, T2: AsRef<Path>>(kernel: T, bootloader: T2) {
    let (kernel, bootloader) = (kernel.as_ref(), bootloader.as_ref());
    let kernel_bin = kernel.with_extension("bin");
    // Use llvm-objcopy. Bootloader requires this.
    let tools =
        LlvmTools::new().expect("Missing llvm tools from the llvm-tools-preview rustup component");
    let objcopy = tools
        .tool(&exe("llvm-objcopy"))
        .expect("Couldn't find llvm-objcopy");
    let _cmd = Command::new(objcopy)
        .arg("-I")
        .arg("elf64-x86-64")
        .arg("-O")
        .arg("binary")
        .arg("--binary-architecture=i386:x86-64")
        .arg(&bootloader)
        .arg(&kernel_bin)
        .spawn()
        .expect("Failed to execute llvm-objcopy");
    //
    const BLOCK_SIZE: u64 = 512;
    let image = OpenOptions::new()
        .write(true)
        .open(kernel_bin)
        .expect("Failed to open kernel.bin");
    // Padding
    let size = image
        .metadata()
        .expect("Couldn't get kernel.bin metadata")
        .len();
    let remain = size % BLOCK_SIZE;
    let padding = if remain > 0 { BLOCK_SIZE - remain } else { 0 };
    image
        .set_len((size + padding) as u64)
        .expect("Failed to pad kernel image");
    //
    image.sync_all().unwrap();
}

fn parse_args() {
    App::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .bin_name("cargo")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::GlobalVersion)
        .subcommand(SubCommand::with_name("image").about(crate_description!()))
        .get_matches();
}

fn main() {
    parse_args();
    //
    let meta = MetadataCommand::new()
        .exec()
        .expect("Unable to read Cargo.toml");
    //
    println!("======Building kernel======");
    let kernel = build_kernel(&meta);
    println!("====Building bootloader====");
    let boot_out = build_bootloader(&meta, &kernel);
    // Combine Kernel and Bootloader
    println!("======Creating image======");
    create_image(&kernel, &boot_out);
}
