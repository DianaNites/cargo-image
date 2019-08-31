use cargo_metadata::{Metadata, MetadataCommand, Package};
use llvm_tools::{exe, LlvmTools};
use std::{
    env,
    fs::OpenOptions,
    path::{Path, PathBuf},
    process::Command,
};
use structopt::{clap::AppSettings, StructOpt};

#[derive(StructOpt, Debug)]
#[structopt(
    bin_name = "cargo",
    global_settings(&[
        AppSettings::ColoredHelp,
]))]
enum Args {
    Image(Image),
}

#[derive(StructOpt, Debug)]
struct Image {
    /// Target directory
    #[structopt(long, default_value = "./target", env = "CARGO_TARGET_DIR")]
    target_dir: PathBuf,

    /// Path to `Cargo.toml`
    #[structopt(long, default_value = "./Cargo.toml")]
    manifest_path: PathBuf,

    /// Whether to build in release mode.
    #[structopt(long)]
    release: bool,

    /// Name of the kernel crate, for when using workspaces.
    #[structopt(long)]
    kernel_crate: Option<String>,
}

/// Returns path to the bootloader binary.
fn build_bootloader(meta: &Metadata, kernel_image: &Path, kernel_crate: &Package) -> PathBuf {
    let bootloader: &Package = meta
        .packages
        .iter()
        .find(|x| x.name == "bootloader")
        .expect("Missing bootloader dependency");
    let bootloader_manifest = Path::new(&bootloader.manifest_path);
    let bootloader_target = bootloader_manifest.with_file_name("x86_64-bootloader.json");
    let bootloader_triple = bootloader_target
        .file_stem()
        .expect("Couldn't parse bootloader target triple");
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
    let mut cmd = Command::new(&cargo);
    cmd.arg("build")
        // Required now, for some reason.
        .env("KERNEL", kernel_image)
        .env("KERNEL_MANIFEST", &kernel_crate.manifest_path)
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
        cmd.arg("--features").arg("binary");
    }
    let exit = cmd.status().expect("Failed to build bootloader");
    assert!(exit.success(), "Failed to build bootloader");

    bootloader_manifest
        .with_file_name("target")
        .join(bootloader_triple)
        .join("release")
        .join("bootloader")
}

/// Returns path to the kernel binary
fn build_kernel(args: &Image, kernel_crate: &Package) -> PathBuf {
    let self_bin = kernel_crate
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
    let target_triple = target.file_stem().expect("Couldn't parse target triple");

    // Build sysroot, just in case.
    let exit = Command::new(&cargo)
        .arg("sysroot")
        .status()
        .expect("Failed to build kernel sysroot");
    assert!(exit.success(), "Failed to build kernel sysroot");

    // Build kernel
    let mut cmd = Command::new(&cargo);
    cmd.arg("build");
    if args.release {
        cmd.arg("--release");
    }
    let exit = cmd.status().expect("Failed to build kernel");
    assert!(exit.success(), "Failed to build kernel");

    let mut final_path: PathBuf = PathBuf::from(&args.target_dir).join(target_triple);
    if args.release {
        final_path = final_path.join("release")
    } else {
        final_path = final_path.join("debug")
    }
    final_path.join(&self_bin.name)
}

/// Creates the final image by combining the bootloader and the kernel.
fn create_image(kernel: &Path, bootloader: &Path) {
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

fn main() {
    let Args::Image(args) = Args::from_args();
    //
    let meta = MetadataCommand::new()
        .manifest_path(&args.manifest_path)
        .exec()
        .expect("Unable to read Cargo.toml");
    //
    let kernel_crate: &Package = {
        if meta.workspace_members.len() == 1 {
            &meta[&meta.workspace_members[0]]
        } else {
            let mut found = None;
            let kernel_crate_name = args
                .kernel_crate
                .as_ref()
                .expect("Must provide `--kernel-crate` when using virtual manifest");
            for member in &meta.workspace_members {
                let pkg = &meta[member];
                if pkg.name == *kernel_crate_name {
                    found = Some(pkg);
                    break;
                }
            }
            found.expect(&format!(
                "Couldn't find kernel crate `{}`",
                kernel_crate_name
            ))
        }
    };

    println!("======Building kernel======");
    let kernel = build_kernel(&args, kernel_crate);

    println!("====Building bootloader====");
    let boot_out = build_bootloader(&meta, &kernel, kernel_crate);

    // Combine Kernel and Bootloader
    println!("======Creating image======");
    create_image(&kernel, &boot_out);
}
