//! Archivos de terceros que el celular descarga, fijados con su huella SHA-256 y su tamaño.
//! Nada se usa sin comprobarlo (igual que el agente de Playit en Windows y Linux).
//!
//! - **Java para Android**: el JRE que publica AngelAuraMC (sucesor de PojavLauncher) en
//!   github.com/AngelAuraMC/angelauramc-openjdk-build, releases `download_jre17` y
//!   `download_jre21`. Es OpenJDK compilado para Android (GPL-2.0 con la Classpath Exception,
//!   ver `legal/java.base/LICENSE` dentro del paquete). Es el mismo origen que usa el lanzador
//!   Amethyst en su gestor de JRE. Huellas calculadas de los archivos descargados.
//! - **Playit**: el agente oficial (`playitd`, BSD-2-Clause) compilado para Android desde el
//!   código de la etiqueta v1.2.0 de playit-cloud/playit-agent (commit fijado abajo). No se usa
//!   el binario `playit-linux-aarch64` publicado: está enlazado con musl, que en Android busca
//!   `/etc/resolv.conf` (no existe) y se quedaría sin DNS. En Android no se puede ejecutar nada
//!   desde la carpeta de datos de la app, así que va dentro del APK como `libplayit.so` (la
//!   carpeta de bibliotecas nativas es la única desde la que Android deja ejecutar).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Asset {
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

/// JRE 21 para Android aarch64 (`JAVA_VERSION="21.0.1"` en su archivo `release`).
pub const JRE21_ARM64: Asset = Asset {
    url: "https://github.com/AngelAuraMC/angelauramc-openjdk-build/releases/download/download_jre21/jre21-android-arm64.tar.xz",
    sha256: "8d41ec401ee59f7722df60ed991f81ad146e130452804bfdd8a05d3436f7bbfe",
    size: 28_675_516,
};

/// JRE 17 para Android aarch64.
pub const JRE17_ARM64: Asset = Asset {
    url: "https://github.com/AngelAuraMC/angelauramc-openjdk-build/releases/download/download_jre17/jre17-android-arm64.tar.xz",
    sha256: "e162c860fe05ee4a4e4af7606437419879f6c748386a7b09fa77d10db6a64091",
    size: 26_900_804,
};

/// Código del agente de Playit que el workflow de Android compila (etiqueta y commit exactos).
pub const PLAYIT_SOURCE_REPO: &str = "https://github.com/playit-cloud/playit-agent.git";
pub const PLAYIT_SOURCE_TAG: &str = "v1.2.0";
pub const PLAYIT_SOURCE_COMMIT: &str = "7acf13568a99707dda73028234fdb376e45b15da";

/// Nombre con el que el agente va dentro del APK (en `jniLibs/arm64-v8a/`).
pub const PLAYIT_LIB_NAME: &str = "libplayit.so";

/// Arquitectura del celular en la que esto está preparado.
pub fn supported_arch(arch: &str) -> bool {
    matches!(arch, "aarch64" | "arm64" | "arm64-v8a")
}

/// Java que necesita una versión de Minecraft en el celular: 21 (1.20.5 en adelante) o 17
/// (1.18 a 1.20.4). Las anteriores piden Java 8 o 16 y no se ofrecen en el celular.
pub fn jre_major_for(mc_version: &str) -> Option<u32> {
    let req = merc_core::java::required_java(mc_version)?;
    match req.min_major {
        21.. => Some(21),
        17..=20 => Some(17),
        _ => None,
    }
}

/// El JRE verificado para esa versión de Java, si lo hay para esta arquitectura.
pub fn jre(major: u32, arch: &str) -> Option<Asset> {
    if !supported_arch(arch) {
        return None;
    }
    match major {
        21 => Some(JRE21_ARM64),
        17 => Some(JRE17_ARM64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_per_version() {
        assert_eq!(jre_major_for("1.21.1"), Some(21));
        assert_eq!(jre_major_for("1.20.5"), Some(21));
        assert_eq!(jre_major_for("1.20.4"), Some(17));
        assert_eq!(jre_major_for("1.18.2"), Some(17));
        assert_eq!(jre_major_for("1.17.1"), None);
        assert_eq!(jre_major_for("1.12.2"), None);
    }

    #[test]
    fn assets_are_pinned() {
        for a in [JRE21_ARM64, JRE17_ARM64] {
            assert!(a.url.starts_with("https://github.com/"), "{}", a.url);
            assert_eq!(a.sha256.len(), 64);
            assert!(a.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(a.size > 1_000_000);
        }
        assert_eq!(PLAYIT_SOURCE_TAG, format!("v{}", merc_net::playit::AGENT_VERSION));
        assert_eq!(PLAYIT_SOURCE_COMMIT.len(), 40);
        assert_eq!(jre(21, "aarch64"), Some(JRE21_ARM64));
        assert_eq!(jre(17, "arm64-v8a"), Some(JRE17_ARM64));
        assert_eq!(jre(21, "x86_64"), None);
        assert_eq!(jre(8, "aarch64"), None);
    }
}
