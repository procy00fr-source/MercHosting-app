//! Cuánta memoria darle al servidor en el celular. Android necesita la suya para el sistema y
//! la propia app, y si el celular se queda sin memoria cierra procesos (el servidor el primero),
//! así que se deja margen de sobra y se pone un techo bajo.

use serde::Serialize;

/// Por debajo de esto no se ofrece crear servidores: no quedaría memoria para el sistema.
pub const MIN_DEVICE_MB: u64 = 3_000;
/// Lo mínimo con lo que arranca un servidor pequeño.
pub const MIN_HEAP_MB: u32 = 1_024;
/// Techo absoluto en el celular.
pub const MAX_HEAP_MB: u32 = 6_144;
/// Memoria que siempre se deja libre para Android y las apps.
pub const RESERVED_MB: u64 = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HeapLimits {
    /// Lo que se propone por defecto.
    pub suggested_mb: u32,
    /// Lo máximo que se deja elegir.
    pub max_mb: u32,
}

fn round_down(mb: u64) -> u32 {
    ((mb / 256) * 256) as u32
}

/// Límites para un celular con `total_mb` de memoria. `None` si tiene demasiado poca.
pub fn heap_limits(total_mb: u64) -> Option<HeapLimits> {
    if total_mb < MIN_DEVICE_MB {
        return None;
    }
    let by_share = total_mb * 45 / 100;
    let by_reserve = total_mb.saturating_sub(RESERVED_MB);
    let max = round_down(by_share.min(by_reserve)).min(MAX_HEAP_MB);
    if max < MIN_HEAP_MB {
        return None;
    }
    let suggested = round_down(total_mb / 4).clamp(MIN_HEAP_MB, max);
    Some(HeapLimits {
        suggested_mb: suggested,
        max_mb: max,
    })
}

/// Ajusta lo que pide la persona a los límites del celular.
pub fn clamp_heap(requested_mb: u32, limits: &HeapLimits) -> u32 {
    let r = (requested_mb / 256) * 256;
    r.clamp(MIN_HEAP_MB, limits.max_mb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_by_device() {
        assert_eq!(heap_limits(2_048), None);
        assert_eq!(heap_limits(2_999), None);
        let four = heap_limits(4_096).unwrap();
        assert_eq!(
            four,
            HeapLimits {
                suggested_mb: 1_024,
                max_mb: 1_792
            }
        );
        let eight = heap_limits(8_192).unwrap();
        assert_eq!(
            eight,
            HeapLimits {
                suggested_mb: 2_048,
                max_mb: 3_584
            }
        );
        let sixteen = heap_limits(16_384).unwrap();
        assert_eq!(sixteen.max_mb, MAX_HEAP_MB);
        assert_eq!(sixteen.suggested_mb, 4_096);
        // Con 3 GB queda poco: el máximo apenas supera el mínimo.
        let three = heap_limits(3_072).unwrap();
        assert_eq!(three.max_mb, 1_024);
        assert_eq!(three.suggested_mb, 1_024);
    }

    #[test]
    fn clamps_requests() {
        let l = heap_limits(8_192).unwrap();
        assert_eq!(clamp_heap(100, &l), MIN_HEAP_MB);
        assert_eq!(clamp_heap(99_999, &l), l.max_mb);
        assert_eq!(clamp_heap(2_300, &l), 2_048);
    }
}
