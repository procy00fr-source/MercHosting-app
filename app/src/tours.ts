// Tutoriales: cada paso señala un control real de la app (por selector CSS) y, si hace falta,
// lleva a la pantalla donde está. Ningún paso simula nada: si el control no está (por ejemplo,
// el servidor está apagado y no hay botón «Detener»), el paso lo explica igual.

export type TourNav =
  | { view: "servers" | "companions" | "appearance" | "supporter" | "help" }
  /** Abre un servidor (el actual o el primero) en una pestaña. «contenido» = Mods o Plugins. */
  | { server: true; tab: string };

export interface TourStep {
  nav?: TourNav;
  /** Selector del control que se resalta. Sin él, el paso sale centrado. */
  target?: string;
  title: string;
  body: string;
  /** El paso avanza solo cuando haces clic en el control resaltado. */
  advanceOnClick?: boolean;
}

export interface Tour {
  id: string;
  name: string;
  description: string;
  /** Necesita al menos un servidor. */
  needsServer?: boolean;
  steps: TourStep[];
}

const nav = (tab: string): TourNav => ({ server: true, tab });

export const TOURS: Tour[] = [
  {
    id: "primeros-pasos",
    name: "Primeros pasos",
    description: "Dónde está cada cosa en MercHosting.",
    steps: [
      {
        nav: { view: "servers" },
        target: '[data-tour="nav-servers"]',
        title: "Tus servidores",
        body: "Aquí ves todos tus servidores y si están encendidos. Cada servidor es una carpeta en tu PC.",
      },
      {
        target: '[data-tour="crear-servidor"]',
        title: "Crear un servidor",
        body: "MercHosting descarga el servidor desde su fuente oficial, comprueba que no esté dañado y lo deja listo.",
      },
      {
        target: '[data-tour="importar-servidor"]',
        title: "¿Ya tienes uno?",
        body: "Si ya tienes un servidor en una carpeta, impórtalo: MercHosting no mueve ni cambia tus archivos.",
      },
      {
        target: '[data-tour="nav-companions"]',
        title: "Compañeros",
        body: "Amigos con MercHosting pueden ayudarte a administrar tus servidores desde su PC, con los permisos que elijas.",
      },
      {
        target: '[data-tour="nav-appearance"]',
        title: "Apariencia",
        body: "Modo claro u oscuro y el fondo. No cambia nada de tus servidores.",
      },
      {
        target: '[data-tour="ayuda"]',
        title: "Ayuda y tutoriales",
        body: "Desde aquí vuelves a ver cualquier tutorial, la bienvenida y las preguntas frecuentes.",
      },
    ],
  },
  {
    id: "crear-servidor",
    name: "Crear tu primer servidor",
    description: "El asistente paso a paso.",
    steps: [
      {
        nav: { view: "servers" },
        target: '[data-tour="crear-servidor"]',
        title: "Abre el asistente",
        body: "Haz clic en «Crear servidor» para empezar.",
        advanceOnClick: true,
      },
      {
        target: '[data-tour="wizard-pasos"]',
        title: "Los pasos",
        body: "Tipo de servidor, versión, Java y memoria, ajustes del mundo y resumen. Puedes volver atrás cuando quieras.",
      },
      {
        target: '[data-tour="wizard-tipo"]',
        title: "Elige el tipo",
        body: "Vanilla es Minecraft tal cual. Paper y Purpur aceptan plugins. Fabric, Forge y NeoForge aceptan mods. Si no sabes, Paper es una buena opción para jugar con amigos.",
      },
      {
        title: "La EULA es tuya",
        body: "Al final tendrás que aceptar la EULA de Minecraft tú mismo: MercHosting nunca la acepta por ti. Después descarga e instala con progreso real.",
      },
    ],
  },
  {
    id: "iniciar",
    name: "Encender y usar la consola",
    description: "Iniciar, ver el estado real y enviar comandos.",
    needsServer: true,
    steps: [
      {
        nav: nav("overview"),
        target: '[data-tour="estado"]',
        title: "Estado real",
        body: "«En línea» solo aparece cuando el servidor acepta jugadores de verdad, no solo porque el proceso arrancó.",
      },
      {
        target: '[data-tour="encendido"]',
        title: "Encender y apagar",
        body: "Iniciar, Reiniciar y Detener. Detener guarda el mundo antes de cerrar; forzar el cierre solo se usa si se cuelga.",
      },
      {
        nav: nav("console"),
        target: '[aria-label="Consola del servidor"]',
        title: "La consola",
        body: "Lo que escribe el servidor, en vivo. Abajo puedes enviar comandos de Minecraft (sin la barra «/»). Con las flechas ↑ ↓ repites comandos anteriores.",
      },
      {
        title: "Puedes cerrar la ventana",
        body: "Tus servidores siguen encendidos aunque cierres MercHosting. Al volver a abrirlo, recupera el control.",
      },
    ],
  },
  {
    id: "conexion",
    name: "Que tus amigos entren",
    description: "Misma red, por Internet con Playit y cómo probar la dirección.",
    needsServer: true,
    steps: [
      {
        nav: nav("connection"),
        target: '[aria-label="En tu misma red"]',
        title: "En tu casa",
        body: "Quien esté en tu misma red (tu wifi) entra con esta dirección. Cópiala y pásala.",
      },
      {
        target: '[aria-label="Amigos por Internet"]',
        title: "Amigos por Internet",
        body: "Con Playit.gg (gratis, con tu propia cuenta) tus amigos entran sin abrir puertos del router. MercHosting instala y maneja su agente.",
      },
      {
        target: '[aria-label="Estado de la conexión"]',
        title: "Si algo falla",
        body: "Cada parte de la conexión tiene su estado: servidor, agente, túnel y la prueba desde Internet. Así sabes dónde está el problema.",
      },
    ],
  },
  {
    id: "voz",
    name: "Chat de voz",
    description: "Simple Voice Chat y su puerto UDP.",
    needsServer: true,
    steps: [
      {
        nav: nav("connection"),
        target: '[aria-label="Chat de voz"]',
        title: "Simple Voice Chat",
        body: "Si el servidor tiene Simple Voice Chat, aquí ves su puerto real y si la voz llega por Internet. Si no está instalado, esta parte no aparece.",
      },
      {
        title: "Cada jugador lo necesita",
        body: "Para hablar y oír, cada jugador necesita también Simple Voice Chat en su Minecraft. Un túnel activo no garantiza el audio: prueba con un amigo.",
      },
    ],
  },
  {
    id: "contenido",
    name: "Mods y plugins",
    description: "Buscar, instalar, desactivar y actualizar.",
    needsServer: true,
    steps: [
      {
        nav: nav("contenido"),
        target: '[aria-label="Vista"]',
        title: "Instalados y catálogo",
        body: "Cambia entre lo que ya tienes y el catálogo de Modrinth para buscar más.",
      },
      {
        title: "Solo lo compatible",
        body: "El catálogo solo muestra lo que sirve para tu versión y tipo de servidor, y deja fuera los mods que son solo del juego (cliente).",
      },
      {
        title: "Con cuidado",
        body: "Los mods y plugins son programas que se ejecutan en tu PC: instala solo desde fuentes de confianza. Desactivar es reversible y lo viejo va a la papelera.",
      },
    ],
  },
  {
    id: "copias",
    name: "Copias de seguridad",
    description: "Hacer, comprobar y restaurar copias.",
    needsServer: true,
    steps: [
      {
        nav: nav("backups"),
        target: '[data-tour="hacer-copia"]',
        title: "Haz una copia",
        body: "Del mundo o del servidor completo. También con el servidor encendido: MercHosting pausa el guardado mientras copia.",
      },
      {
        target: '[aria-label="Copias guardadas"]',
        title: "Tus copias",
        body: "Cada copia se comprueba con su huella. Al restaurar, antes se hace otra copia automática, por si te arrepientes.",
      },
      {
        title: "Automáticas",
        body: "En Tareas programadas (modo avanzado) puedes hacer copias cada día sin acordarte.",
      },
    ],
  },
  {
    id: "companeros",
    name: "Compañeros y permisos",
    description: "Dar acceso a un amigo de forma segura.",
    steps: [
      {
        nav: { view: "companions" },
        target: '[aria-label="Tu PC"]',
        title: "Tu PC",
        body: "Primero permites el acceso de compañeros. Puedes pausarlo para todos cuando quieras.",
      },
      {
        title: "Invitar",
        body: "Creas un código de un solo uso para un servidor y eliges qué puede hacer: ver estado, encender, leer la consola… Cada permiso va por separado.",
      },
      {
        title: "Archivos reales",
        body: "Dar acceso a archivos, comandos o mods es de alto riesgo: son archivos reales de tu PC. Dalo solo a personas de confianza. Todo queda en la Actividad.",
      },
    ],
  },
];

export function tourById(id: string): Tour | undefined {
  return TOURS.find((t) => t.id === id);
}
