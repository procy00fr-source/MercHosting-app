//! Lanzador de la JVM dentro del proceso actual, como hace PojavLauncher/Amethyst en Android.
//!
//! Desde Android 10 una app no puede ejecutar (`execve`) binarios guardados en su carpeta de
//! datos, así que no se puede lanzar `bin/java` como en una PC. En su lugar se cargan las
//! bibliotecas del JRE con `dlopen` y se llama a `JLI_Launch` (el «main» del lanzador de
//! OpenJDK) en un proceso aparte de la app (`:server`, un servicio en primer plano). La salida
//! del servidor va a un archivo de registro y los comandos entran por una tubería con nombre.
//! Cuando el servidor termina, JLI cierra ese proceso: la app no se ve afectada.
//!
//! En Linux de escritorio funciona igual con un JRE normal (lo usan las pruebas).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Todo lo que necesita el proceso del servidor para arrancar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchSpec {
    /// Carpeta del JRE (con `lib/libjli.so` y `lib/server/libjvm.so`).
    pub jre: PathBuf,
    /// Carpeta del servidor (directorio de trabajo).
    pub work: PathBuf,
    /// Archivo donde se escribe toda la salida.
    pub log: PathBuf,
    /// Tubería con nombre por la que entran los comandos (stdin).
    pub fifo: PathBuf,
    /// Archivo con el PID mientras corre.
    pub pid_file: PathBuf,
    /// Argumentos completos, empezando por `java`.
    pub args: Vec<String>,
    /// Programa que acompaña al servidor (el agente de Playit). Se cierra con este proceso.
    #[serde(default)]
    pub companion: Option<Companion>,
}

/// Programa auxiliar que se arranca junto al servidor y muere con él.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Companion {
    pub exe: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("{0}")]
    Io(String),
    #[error("no se pudo cargar {0}: {1}")]
    Dlopen(String, String),
    #[error("el JRE no trae JLI_Launch")]
    NoSymbol,
}

/// Bibliotecas del JRE que se cargan antes, en orden (como Amethyst).
pub const PRELOAD: [&str; 7] = [
    "lib/libjli.so",
    "lib/server/libjvm.so",
    "lib/libverify.so",
    "lib/libjava.so",
    "lib/libzip.so",
    "lib/libnet.so",
    "lib/libnio.so",
];

#[cfg(unix)]
mod imp {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int};
    use std::os::unix::ffi::OsStrExt;

    type JliLaunch = unsafe extern "C" fn(
        argc: c_int,
        argv: *mut *mut c_char,
        jargc: c_int,
        jargv: *const *const c_char,
        appclassc: c_int,
        appclassv: *const *const c_char,
        fullversion: *const c_char,
        dotversion: *const c_char,
        pname: *const c_char,
        lname: *const c_char,
        javaargs: u8,
        cpwildcard: u8,
        javaw: u8,
        ergo: c_int,
    ) -> c_int;

    fn cpath(p: &std::path::Path) -> Result<CString, LaunchError> {
        CString::new(p.as_os_str().as_bytes()).map_err(|_| LaunchError::Io(format!("ruta no válida: {}", p.display())))
    }

    fn dl_error() -> String {
        // SAFETY: dlerror devuelve una cadena estática o NULL.
        unsafe {
            let e = libc::dlerror();
            if e.is_null() {
                String::new()
            } else {
                CStr::from_ptr(e).to_string_lossy().into_owned()
            }
        }
    }

    fn dlopen_global(p: &std::path::Path) -> Result<*mut libc::c_void, LaunchError> {
        let c = cpath(p)?;
        // SAFETY: ruta terminada en NUL; RTLD_GLOBAL para que libjvm vea a libjli y al revés.
        let h = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };
        if h.is_null() {
            return Err(LaunchError::Dlopen(p.display().to_string(), dl_error()));
        }
        Ok(h)
    }

    /// En Android el enlazador no mira `LD_LIBRARY_PATH` cambiado en marcha: hay que avisarle.
    #[cfg(target_os = "android")]
    fn update_library_path(jre: &std::path::Path) {
        let path = format!(
            "{}:{}:{}",
            jre.join("lib").display(),
            jre.join("lib/server").display(),
            jre.join("lib/jli").display()
        );
        let Ok(cp) = CString::new(path) else { return };
        // SAFETY: búsqueda de símbolos en libdl; se llama solo si existe.
        unsafe {
            let libdl = libc::dlopen(c"libdl.so".as_ptr(), libc::RTLD_LAZY);
            if libdl.is_null() {
                return;
            }
            let mut f = libc::dlsym(libdl, c"android_update_LD_LIBRARY_PATH".as_ptr());
            if f.is_null() {
                f = libc::dlsym(libdl, c"__loader_android_update_LD_LIBRARY_PATH".as_ptr());
            }
            if !f.is_null() {
                let update: unsafe extern "C" fn(*const c_char) = std::mem::transmute(f);
                update(cp.as_ptr());
            }
        }
    }

    /// Arranca el auxiliar sin entrada ni salida y con la orden de cerrarse si este proceso
    /// muere (`PR_SET_PDEATHSIG`), para que no quede suelto cuando el servidor termina.
    fn spawn_companion(c: &Companion) -> std::io::Result<u32> {
        use std::os::unix::process::CommandExt;
        std::fs::create_dir_all(&c.cwd)?;
        let mut cmd = std::process::Command::new(&c.exe);
        cmd.args(&c.args)
            .current_dir(&c.cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        // SAFETY: solo llamadas seguras tras fork (prctl).
        unsafe {
            cmd.pre_exec(|| {
                #[cfg(any(target_os = "linux", target_os = "android"))]
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        Ok(cmd.spawn()?.id())
    }

    /// Prepara la salida, la entrada y el directorio, carga el JRE y llama a `JLI_Launch`.
    /// Devuelve el código de salida si JLI vuelve (normalmente termina el proceso).
    pub fn launch_in_process(spec: &LaunchSpec) -> Result<i32, LaunchError> {
        let io = |e: std::io::Error| LaunchError::Io(e.to_string());
        std::fs::create_dir_all(&spec.work).map_err(io)?;
        // Tubería de comandos: se abre de lectura y escritura para que no dé fin de archivo
        // cuando la app cierra su lado entre comando y comando.
        if !spec.fifo.exists() {
            let c = cpath(&spec.fifo)?;
            // SAFETY: ruta válida.
            if unsafe { libc::mkfifo(c.as_ptr(), 0o600) } != 0 {
                return Err(LaunchError::Io(format!("no se pudo crear {}", spec.fifo.display())));
            }
        }
        let fifo = cpath(&spec.fifo)?;
        let log = cpath(&spec.log)?;
        let work = cpath(&spec.work)?;
        // SAFETY: descriptores recién abiertos; dup2 sobre 0, 1 y 2 de este proceso.
        unsafe {
            let fin = libc::open(fifo.as_ptr(), libc::O_RDWR);
            let fout = libc::open(
                log.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_APPEND,
                0o600,
            );
            if fin < 0 || fout < 0 {
                return Err(LaunchError::Io("no se pudo abrir la consola del servidor".into()));
            }
            libc::dup2(fin, 0);
            libc::dup2(fout, 1);
            libc::dup2(fout, 2);
            if libc::chdir(work.as_ptr()) != 0 {
                return Err(LaunchError::Io("no se pudo entrar en la carpeta del servidor".into()));
            }
        }
        std::fs::write(&spec.pid_file, std::process::id().to_string()).map_err(io)?;
        if let Some(c) = &spec.companion {
            use std::io::Write;
            let line = match spawn_companion(c) {
                Ok(pid) => format!("[MercHosting] Playit en marcha (pid {pid})"),
                Err(e) => format!("[MercHosting] No se pudo arrancar Playit: {e}"),
            };
            // Directo a la salida 1 (la consola del servidor), sin pasar por println!.
            let mut out = std::io::stdout();
            let _ = writeln!(out, "{line}");
            let _ = out.flush();
        }

        #[cfg(target_os = "android")]
        update_library_path(&spec.jre);

        let mut jli = std::ptr::null_mut();
        for (i, rel) in PRELOAD.iter().enumerate() {
            let p = spec.jre.join(rel);
            if !p.is_file() {
                if i < 2 {
                    return Err(LaunchError::Dlopen(p.display().to_string(), "no existe".into()));
                }
                continue;
            }
            let h = dlopen_global(&p)?;
            if i == 0 {
                jli = h;
            }
        }
        // SAFETY: símbolo de libjli con la firma de OpenJDK (java_md / main.c).
        let launch: JliLaunch = unsafe {
            let s = libc::dlsym(jli, c"JLI_Launch".as_ptr());
            if s.is_null() {
                return Err(LaunchError::NoSymbol);
            }
            std::mem::transmute::<*mut libc::c_void, JliLaunch>(s)
        };
        let owned: Vec<CString> = spec
            .args
            .iter()
            .map(|a| CString::new(a.as_str()).map_err(|_| LaunchError::Io("argumento no válido".into())))
            .collect::<Result<_, _>>()?;
        let mut argv: Vec<*mut c_char> = owned.iter().map(|c| c.as_ptr() as *mut c_char).collect();
        argv.push(std::ptr::null_mut());
        let name = c"java";
        let launcher = c"openjdk";
        // SAFETY: argv vive hasta el final de la llamada; versiones como las de Amethyst.
        let code = unsafe {
            launch(
                owned.len() as c_int,
                argv.as_mut_ptr(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                c"1.8.0-internal".as_ptr(),
                c"1.8".as_ptr(),
                name.as_ptr(),
                launcher.as_ptr(),
                0,
                1,
                0,
                0,
            )
        };
        let _ = std::fs::remove_file(&spec.pid_file);
        Ok(code)
    }
}

#[cfg(unix)]
pub use imp::launch_in_process;

/// Entrada del servicio de Android (por JNI): lee la especificación en JSON y lanza. Si algo
/// falla antes de la JVM, lo escribe en la consola del servidor para que la app lo muestre.
#[cfg(unix)]
pub fn launch_from_json(json: &str) -> i32 {
    let spec: LaunchSpec = match serde_json::from_str(json) {
        Ok(s) => s,
        Err(_) => return 98,
    };
    match launch_in_process(&spec) {
        Ok(code) => {
            use std::io::Write;
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&spec.log)
                .and_then(|mut f| writeln!(f, "[MercHosting] El servidor terminó (código {code})."));
            let _ = std::fs::remove_file(&spec.pid_file);
            code
        }
        Err(e) => {
            use std::io::Write;
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&spec.log)
                .and_then(|mut f| writeln!(f, "[MercHosting] No se pudo encender el servidor: {e}"));
            let _ = std::fs::remove_file(&spec.pid_file);
            99
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un JRE de este sistema con `libjli.so` (en Linux, el de la distribución).
    #[cfg(all(unix, not(target_os = "android")))]
    fn system_jre() -> Option<PathBuf> {
        let dir = std::fs::read_dir("/usr/lib/jvm").ok()?;
        let mut v: Vec<PathBuf> = dir
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.join("lib/libjli.so").is_file() && p.join("lib/server/libjvm.so").is_file())
            .collect();
        v.sort();
        v.pop()
    }

    /// Lanza `java -version` de verdad con dlopen + JLI_Launch en un proceso hijo (fork), con la
    /// salida al registro: es exactamente el camino que usa el celular.
    #[cfg(all(unix, not(target_os = "android")))]
    #[test]
    fn real_jvm_runs_in_process() {
        let Some(jre) = system_jre() else {
            eprintln!("sin JRE en /usr/lib/jvm: prueba omitida");
            return;
        };
        let d = tempfile::tempdir().unwrap();
        let spec = LaunchSpec {
            jre,
            work: d.path().join("srv"),
            log: d.path().join("consola.log"),
            fifo: d.path().join("entrada"),
            pid_file: d.path().join("srv.pid"),
            args: vec!["java".into(), "-Xmx64M".into(), "-version".into()],
            companion: Some(Companion {
                exe: "/bin/sh".into(),
                args: vec![
                    "-c".into(),
                    format!("echo $$ > {}; exec sleep 600", d.path().join("aux.pid").display()),
                ],
                cwd: d.path().join("aux"),
            }),
        };
        // SAFETY: el hijo solo lanza la JVM y sale; el padre espera.
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            let code = match launch_in_process(&spec) {
                Ok(c) => c,
                Err(e) => {
                    let _ = std::fs::write(d.path().join("error"), e.to_string());
                    99
                }
            };
            // SAFETY: salir del hijo sin correr destructores del arnés de pruebas.
            unsafe { libc::_exit(code) };
        }
        let mut status = 0;
        // SAFETY: pid del hijo recién creado.
        unsafe { libc::waitpid(pid, &mut status, 0) };
        let err = std::fs::read_to_string(d.path().join("error")).unwrap_or_default();
        assert!(err.is_empty(), "{err}");
        let log = std::fs::read_to_string(&spec.log).unwrap();
        assert!(log.contains("version"), "{log}");
        assert!(
            libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0,
            "{status} {log}"
        );
        assert!(!spec.pid_file.exists());
        assert!(log.contains("Playit en marcha"), "{log}");
        // El auxiliar muere con el proceso del servidor.
        let aux: u32 = loop {
            if let Some(p) = std::fs::read_to_string(d.path().join("aux.pid"))
                .ok()
                .and_then(|t| t.trim().parse().ok())
            {
                break p;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        let gone = (0..100).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let st = std::fs::read_to_string(format!("/proc/{aux}/stat")).unwrap_or_default();
            st.is_empty() || st.split_whitespace().nth(2) == Some("Z")
        });
        assert!(gone, "el auxiliar sigue vivo");
    }

    #[cfg(all(unix, not(target_os = "android")))]
    #[test]
    fn missing_jre_is_reported() {
        let d = tempfile::tempdir().unwrap();
        let spec = LaunchSpec {
            jre: d.path().join("no-hay"),
            work: d.path().join("srv"),
            log: d.path().join("consola.log"),
            fifo: d.path().join("entrada"),
            pid_file: d.path().join("srv.pid"),
            args: vec!["java".into()],
            companion: None,
        };
        // En un hijo, porque cambia la entrada y la salida del proceso.
        // SAFETY: igual que arriba.
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            let ok = matches!(launch_in_process(&spec), Err(LaunchError::Dlopen(..)));
            unsafe { libc::_exit(if ok { 0 } else { 1 }) };
        }
        let mut status = 0;
        unsafe { libc::waitpid(pid, &mut status, 0) };
        assert!(libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0);
    }
}
