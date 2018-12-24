#![allow(dead_code, unused_parens)]
use cargo_metadata::metadata_deps;
use std::{
    env,
    fs::File,
    io::prelude::*,
    mem,
    path::{Path, PathBuf},
    process::Command,
    slice,
};

fn main() {
    // let manifest = Path::new("Cargo.toml");
    // let config = Path::new(".cargo/config");
    let manifest = Path::new("C:/_Diana/Projects/diaos/Cargo.toml");
    let config = Path::new("C:/_Diana/Projects/diaos/.cargo/config");
    //
    let target = PathBuf::from(
        cargo_toml2::from_path::<_, cargo_toml2::CargoConfig>(config)
            .expect("Couldn't read .cargo/config")
            .build
            .expect("Couldn't read target")
            .target
            .expect("Couldn't read target"),
    );
    let target_triple = target.file_stem().expect("Impossible");
    let meta = metadata_deps(Some(manifest), true).expect("Unable to read Cargo.toml");
    let selfa = meta
        .packages
        .iter()
        .find(|x| {
            Path::new(&x.manifest_path)
                .canonicalize()
                .expect("Impossible")
                == manifest.canonicalize().expect("Impossible")
        })
        .expect("Couldn't find self in cargo-metadata");
    println!("{:#?}", selfa);
    let bootloader = meta
        .packages
        .iter()
        .find(|x| x.name == "bootloader")
        .expect("Missing bootloader dependency");
    println!("Bootloader: {:#?}", bootloader);
    let kernel = PathBuf::from(&meta.target_directory)
        .join(target_triple)
        .join("debug")
        .join("diaos");
    println!("Kernel: {:#?}", kernel);
    // Build kernel.
    Command::new(env::var_os("CARGO").expect("Missing CARGO environment variable."))
        .arg("build")
        .current_dir(manifest.parent().unwrap()) // TESTING
        .status()
        .expect("Failed to build kernel");
    let bootloader_triple =
        Path::new(&bootloader.manifest_path).with_file_name("x86_64-bootloader.json");
    // Build bootloader sysroot
    Command::new(env::var_os("CARGO").expect("Missing CARGO environment variable."))
        .arg("sysroot")
        .arg("--target")
        .arg(&bootloader_triple)
        .current_dir(
            Path::new(&bootloader.manifest_path)
                .parent()
                .expect("Impossible"),
        )
        .status()
        .expect("Failed to build bootloader sysroot");
    // Build bootloader
    Command::new(env::var_os("CARGO").expect("Missing CARGO environment variable."))
        .arg("build")
        .arg("--release")
        .arg("--target")
        .arg(&bootloader_triple)
        .current_dir(
            Path::new(&bootloader.manifest_path)
                .parent()
                .expect("Impossible"),
        )
        .status()
        .expect("Failed to build bootloader");
    // Combine Kernel and Bootloader
    let boot_out = Path::new(&bootloader.manifest_path)
        .with_file_name("target")
        .join(bootloader_triple.file_stem().expect("Impossible"))
        .join("release")
        .join("bootloader");
    // panic!("{:#?}", boot_out);
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
