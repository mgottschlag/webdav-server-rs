extern crate cc;

use std::fs::File;
use std::io::prelude::*;
use std::process::Command;

fn run_rpcgen() {
    let res = Command::new("rpcgen")
        .arg("-c")
        .arg("src/rquota.x")
        .output()
        .expect("failed to run rpcgen");
    let csrc = String::from_utf8_lossy(&res.stdout);
    let mut f = File::create("src/rquota_xdr.c").expect("src/rquota_xdr.c");
    f.write_all(
        csrc.replace("/usr/include/rpcsvc/rquota.h", "./rquota.h")
            .replace("src/rquota.h", "./rquota.h")
            .as_bytes(),
    )
    .unwrap();

    let res = Command::new("rpcgen")
        .arg("-h")
        .arg("src/rquota.x")
        .output()
        .expect("failed to run rpcgen");
    let hdr = String::from_utf8_lossy(&res.stdout);
    let mut f = File::create("src/rquota.h").expect("src/rquota.h");
    f.write_all(hdr.as_bytes()).unwrap();
}

fn main() {
    #[cfg(feature = "nfs")]
    run_rpcgen();

    let mut builder = cc::Build::new();

    #[cfg(target_os = "linux")]
    builder.file("src/quota-linux.c");

    #[cfg(feature = "nfs")]
    {
        builder.file("src/quota-nfs.c").file("src/rquota_xdr.c");
    }
    builder
        .flag_if_supported("-Wno-unused-variable")
        .compile("fs_quota");

    println!("cargo:rustc-link-lib=rpcsvc");

    #[cfg(target_os = "linux")]
    println!("cargo:rerun-if-changed=src/quota-linux.c");

    #[cfg(feature = "nfs")]
    {
        println!("cargo:rerun-if-changed=src/rquota.x");
        println!("cargo:rerun-if-changed=src/quota-nfs.c");
    }
}
