# Genera el logo de MercHosting desde su geometría (rejilla isométrica):
#   - app/src/components/logoPaths.ts (piezas para la interfaz)
#   - assets/logo/merchosting-icono.svg (icono de la app: cuadrado redondeado + cubo)
#   - assets/logo/merchosting-cubo.svg (solo el cubo, fondo transparente)
# Uso: python3 scripts/logo/generar-logo.py
# Después, los iconos de Windows: npx tauri icon assets/logo/merchosting-icono-1024.png (desde app/)
import math, os, sys
sys.path.insert(0, os.path.dirname(__file__))
from geometria import build, path

ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
P = build(c=0.26)
K = math.sqrt(3) / 2 * 100
VB = f"{-K-4:.1f} -104 {2*K+8:.1f} 208"
ORDER = ["top", "outerLeft", "outerRight", "core", "innerRight", "innerLeft"]

def pieces(fill="url(#mh-body)", core="url(#mh-core)"):
    return "".join(f'<path d="{path(P[n])}" fill="{core if n == "core" else fill}"/>' for n in ORDER)

DEFS = ('<linearGradient id="mh-body" x1="0" y1="-100" x2="0" y2="100" gradientUnits="userSpaceOnUse">'
        '<stop offset="0" stop-color="#ffffff"/><stop offset="1" stop-color="#c8d2f0"/></linearGradient>'
        '<linearGradient id="mh-core" x1="0" y1="-30" x2="0" y2="-3" gradientUnits="userSpaceOnUse">'
        '<stop offset="0" stop-color="#8fb3ff"/><stop offset="1" stop-color="#4f7dff"/></linearGradient>')

def cube_svg():
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{VB}" width="512" height="588">'
            f'<defs>{DEFS}</defs>{pieces()}</svg>\n')

def icon_svg(size=1024):
    # Cuadrado redondeado azul noche con brillo arriba a la izquierda y borde fino.
    s = size
    inset, r = s * 0.03, s * 0.19
    cube = 0.69 * s  # alto del cubo
    scale = cube / 208
    tx, ty = s / 2, s / 2 + s * 0.01
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {s} {s}" width="{s}" height="{s}"><defs>'
            f'<radialGradient id="bg" cx="0.45" cy="0.42" r="0.75"><stop offset="0" stop-color="#25389a"/>'
            f'<stop offset="0.6" stop-color="#15226a"/><stop offset="1" stop-color="#080d2c"/></radialGradient>'
            f'<filter id="glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="3"/></filter>'
            f'{DEFS}</defs>'
            f'<rect x="{inset}" y="{inset}" width="{s-2*inset}" height="{s-2*inset}" rx="{r}" fill="url(#bg)"/>'
            f'<rect x="{inset+s*0.004}" y="{inset+s*0.004}" width="{s-2*inset-s*0.008}" height="{s-2*inset-s*0.008}" rx="{r}" fill="none" stroke="#4a62c4" stroke-opacity="0.85" stroke-width="{s*0.007}"/>'
            f'<g transform="translate({tx} {ty}) scale({scale:.4f})">'
            f'<path d="{path(P["core"])}" fill="#6c9bff" filter="url(#glow)" opacity="0.8"/>{pieces()}</g></svg>\n')

def ts_file():
    lines = ['// Logo de MercHosting (diseño del autor), redibujado con geometría exacta sobre una rejilla',
             '// isométrica: cubo de tres caras separadas por un hueco constante. Seis piezas para poder',
             '// animarlas por separado. El rombo central ("core") se usa como indicador de estado del servidor.',
             '// Generado con scripts/logo/generar-logo.py (no editar a mano).', '',
             f'export const LOGO_VIEWBOX = "{VB}";',
             '/** Sin transformación: las coordenadas ya están en el espacio del viewBox. */',
             'export const LOGO_TRANSFORM = "translate(0,0)";', '', 'export const LOGO_PIECES = {']
    lines += [f'  {n}: "{path(P[n])}",' for n in ORDER]
    lines += ['} as const;', '', 'export type LogoPiece = keyof typeof LOGO_PIECES;', '']
    return "\n".join(lines)

if __name__ == "__main__":
    out = os.path.join(ROOT, "assets", "logo")
    os.makedirs(out, exist_ok=True)
    open(os.path.join(out, "merchosting-cubo.svg"), "w").write(cube_svg())
    open(os.path.join(out, "merchosting-icono.svg"), "w").write(icon_svg())
    open(os.path.join(ROOT, "app", "src", "components", "logoPaths.ts"), "w").write(ts_file())
    print("ok")
