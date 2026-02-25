use serde::Deserialize;
use std::io::Write;
use std::{env, fs, path::PathBuf};

#[derive(Deserialize)]
struct Manifest {
    signal: Vec<SignalDef>,
}

#[derive(Deserialize)]
struct SignalDef {
    id: String,
}

fn main() {
    println!("cargo:rerun-if-changed=heuristics.toml");

    let toml_src = fs::read_to_string("heuristics.toml").expect("cannot read heuristics.toml");
    let manifest: Manifest = toml::from_str(&toml_src).expect("invalid heuristics.toml");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mut f = fs::File::create(out_dir.join("signal_ids.rs")).unwrap();

    for sig in &manifest.signal {
        let const_name = sig.id.replace('.', "_").to_ascii_uppercase();
        writeln!(f, "pub const {const_name}: &str = \"{}\";", sig.id).unwrap();
    }
}
