//! Huella del protocolo: cambia cuando cambian los mensajes entre la interfaz y el supervisor.
//! Sirve para que la app detecte un supervisor de una versión anterior que sigue en marcha.

fn main() {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for f in ["src/protocol.rs", "src/remote.rs"] {
        println!("cargo:rerun-if-changed={f}");
        let bytes = std::fs::read(f).unwrap_or_default();
        for b in bytes {
            if b == b'\r' {
                continue; // mismo resultado con finales de línea de Windows o Linux
            }
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    println!("cargo:rustc-env=MERC_PROTOCOL_ID={h:016x}");
}
