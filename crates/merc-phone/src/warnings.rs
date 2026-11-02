//! Avisos de riesgo que hay que aceptar uno por uno antes de crear o encender un servidor en el
//! celular. Los textos los muestra la interfaz; aquí están los identificadores que se comprueban.

/// Todos son obligatorios.
pub const REQUIRED: [&str; 7] = [
    // La batería se gasta rápido y se degrada si se usa así a menudo.
    "bateria",
    // El celular se calienta; si se calienta mucho, Android baja la velocidad o cierra cosas.
    "calor",
    // Android puede cerrar el servidor (memoria, ahorro de energía, la marca del celular).
    "cierre",
    // Se recomienda tenerlo cargando y en Wi-Fi.
    "cargador_wifi",
    // Con Playit, el tráfico de los jugadores pasa por Internet: con datos móviles se gastan.
    "datos",
    // Rinde para pocos jugadores; mods y mundos grandes van lentos.
    "rendimiento",
    // Uso intensivo y prolongado: desgaste de batería y almacenamiento.
    "desgaste",
];

/// Los que faltan por aceptar (vacío = todos aceptados).
pub fn missing(accepted: &[String]) -> Vec<&'static str> {
    REQUIRED
        .iter()
        .copied()
        .filter(|id| !accepted.iter().any(|a| a == id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_required() {
        assert_eq!(missing(&[]).len(), REQUIRED.len());
        let some: Vec<String> = vec!["bateria".into(), "calor".into(), "otro".into()];
        assert_eq!(missing(&some).len(), REQUIRED.len() - 2);
        let all: Vec<String> = REQUIRED.iter().map(|s| s.to_string()).collect();
        assert!(missing(&all).is_empty());
    }
}
