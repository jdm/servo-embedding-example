#[cfg(feature = "glutin_window")]
fn main() {
    use gl_generator::{Api, Fallbacks, Profile, Registry};
    use std::env;
    use std::fs::File;
    use std::path::PathBuf;

    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=build.rs");

    let mut file = File::create(&dest.join("gl_bindings.rs")).unwrap();
    Registry::new(Api::Gles2, (3, 3), Profile::Core, Fallbacks::All, ["KHR_debug"])
        .write_bindings(gl_generator::StructGenerator, &mut file)
        .unwrap();
}

#[cfg(feature = "gtk_window")]
fn main() {}
