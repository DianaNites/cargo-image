#![allow(dead_code, unused_parens)]
use cargo_metadata::{metadata_deps, Metadata};
use std::{
    env,
    fs::File,
    io::prelude::*,
    mem,
    path::{Path, PathBuf},
    process::Command,
    slice,
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
        .arg(&bootloader_triple)
        .current_dir(bootloader_manifest.parent().expect("Impossible"))
        .status()
        .expect("Failed to build bootloader sysroot");
    // Build bootloader
    Command::new(&cargo)
        .arg("build")
        .arg("--release")
        .arg("--target")
        .arg(&bootloader_triple)
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
    // TODO: Get and use binary target name.
    PathBuf::from(&meta.target_directory)
        .join(target_triple)
        .join("debug")
        .join("diaos")
}

fn main() {
    let manifest = Path::new("Cargo.toml");
    //
    let meta = metadata_deps(Some(manifest), true).expect("Unable to read Cargo.toml");
    //
    let boot_out = build_bootloader(&meta);
    let kernel = build_kernel(&meta);
    // Combine Kernel and Bootloader
    let mut kraw = Vec::new();
    let mut k = File::open(&kernel).expect("Failed to open kernel file");
    k.read_to_end(&mut kraw)
        .expect("Failed to read kernel file");
    //
    let mut braw = Vec::new();
    let mut bootloader = File::open(boot_out).expect("Failed to open bootoader file");
    bootloader
        .read_to_end(&mut braw)
        .expect("Failed to read bootloader file");
    //
    let mut image_out =
        File::create(kernel.with_extension("bin")).expect("Failed to create final image");
    //
    let elf = xmas_elf::ElfFile::new(&braw).unwrap();
    xmas_elf::header::sanity_check(&elf).unwrap();
    let bootloader_section = elf
        .find_section_by_name(".bootloader")
        .expect("bootloader must have a .bootloader section");
    image_out
        .write_all(&bootloader_section.raw_data(&elf))
        .unwrap();
    // Write kernel info block u32 little endian (kernel_size, 0)
    let mut kinfo = [0u8; 512];
    let ksize = kraw.len();
    let x: &[u8] =
        unsafe { slice::from_raw_parts(&ksize as *const _ as *const u8, mem::size_of::<u32>()) };
    kinfo[0..4].copy_from_slice(x);
    image_out.write_all(&kinfo).unwrap();

    // Write kernel
    image_out.write_all(&kraw).unwrap();

    // Pad file
    let padding = [0u8; 512];
    let padding_size = (padding.len() - ((kraw.len()) % padding.len())) % padding.len();
    image_out.write_all(&padding[..padding_size]).unwrap();
    image_out.sync_all().unwrap();
}
