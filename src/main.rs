#![allow(dead_code, unused_parens)]
use byteorder::{ByteOrder, LittleEndian};
use cargo_metadata::{metadata_deps, Metadata};
use clap::{crate_description, crate_name, crate_version, App, AppSettings, SubCommand};
use std::{
    env,
    fs::File,
    io::prelude::*,
    path::{Path, PathBuf},
    process::Command,
};

/// Returns path to the bootloader binary.
fn build_bootloader(meta: &Metadata) -> PathBuf {
    let bootloader = meta
        .packages
        .iter()
        .find(|x| x.name == "bootloader")
        .expect("Missing bootloader dependency");
    let bootloader_manifest = Path::new(&bootloader.manifest_path);
    let bootloader_target = bootloader_manifest.with_file_name("x86_64-bootloader.json");
    let bootloader_triple = bootloader_target.file_stem().expect("Impossible");
    let cargo = env::var_os("CARGO").expect("Missing CARGO environment variable.");
    // Build bootloader sysroot
    Command::new(&cargo)
        .arg("sysroot")
        .arg("--target")
        .arg(&bootloader_target)
        .current_dir(bootloader_manifest.parent().expect("Impossible"))
        .status()
        .expect("Failed to build bootloader sysroot");
    // Build bootloader
    Command::new(&cargo)
        .arg("build")
        .arg("--release")
        .arg("--target")
        .arg(&bootloader_target)
        .current_dir(bootloader_manifest.parent().expect("Impossible"))
        .status()
        .expect("Failed to build bootloader");
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
    Command::new(&cargo)
        .arg("sysroot")
        .status()
        .expect("Failed to build kernel sysroot");
    // Build kernel
    Command::new(&cargo)
        .arg("build")
        .status()
        .expect("Failed to build kernel");
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
    // Read the kernel
    let k = {
        let mut x = Vec::new();
        File::open(&kernel)
            .expect("Failed to open kernel file")
            .read_to_end(&mut x)
            .expect("Failed to read kernel file");
        x
    };
    // Read the bootloader
    let b = {
        let mut x = Vec::new();
        File::open(&bootloader)
            .expect("Failed to open bootoader file")
            .read_to_end(&mut x)
            .expect("Failed to read bootoader file");
        x
    };
    //
    let mut image =
        File::create(kernel.with_extension("bin")).expect("Failed to create final image");
    //
    let elf = xmas_elf::ElfFile::new(&b).expect("Couldn't parse ELF header");
    xmas_elf::header::sanity_check(&elf).expect("ELF header failed sanity check");
    let bootloader_section = elf
        .find_section_by_name(".bootloader")
        .expect("bootloader must have a .bootloader section");
    // Write bootloader
    image
        .write_all(bootloader_section.raw_data(&elf))
        .expect("Failed writing bootloader data");
    // Write kernel info block u32 little endian (kernel_size, 0)
    assert!(
        k.len() as u64 <= u64::from(u32::max_value()),
        "Kernel is too large."
    );
    let mut kinfo = [0u8; 512];
    LittleEndian::write_u32(&mut kinfo[0..4], k.len() as u32);
    LittleEndian::write_u32(&mut kinfo[8..12], 0);
    image
        .write_all(&kinfo)
        .expect("Failed writing kernel info block");
    // Write kernel
    image.write_all(&k).expect("Failed writing kernel");
    // Padding
    let padding = [0u8; 512];
    let padding_size = (padding.len() - (k.len() % padding.len())) % padding.len();
    image.write_all(&padding[..padding_size]).unwrap();
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
    let manifest = Path::new("Cargo.toml");
    //
    let meta = metadata_deps(Some(manifest), true).expect("Unable to read Cargo.toml");
    //
    let boot_out = build_bootloader(&meta);
    let kernel = build_kernel(&meta);
    // Combine Kernel and Bootloader
    create_image(&kernel, &boot_out);
}
