//! Con el JRE para Android descargado a mano (MERC_JRE_TARXZ=/ruta/jre21-android-arm64.tar.xz):
//! comprueba que se descomprime con las reglas de seguridad del instalador y trae lo que el
//! lanzador necesita. `cargo test -p merc-phone --test jre_android -- --ignored`.

#[test]
#[ignore]
fn android_jre_extracts_with_the_launcher_libs() {
    let Ok(file) = std::env::var("MERC_JRE_TARXZ") else {
        eprintln!("MERC_JRE_TARXZ no definido: se salta");
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let dest = d.path().join("jre");
    merc_install::temurin::extract_tar_xz(std::path::Path::new(&file), &dest).unwrap();
    for rel in merc_phone::jli::PRELOAD {
        assert!(dest.join(rel).is_file(), "falta {rel}");
    }
    assert!(dest.join("lib/modules").is_file());
    let release = std::fs::read_to_string(dest.join("release")).unwrap();
    assert!(release.contains("OS_ARCH=\"aarch64\""), "{release}");
    assert!(dest.join("legal/java.base/LICENSE").exists());
}
