// Sonido de la entrada de MercHosting, sintetizado con Web Audio (sin archivos de terceros).
// Los tiempos coinciden con la animación del cubo en styles.css (.logo-assemble y .splash-word).

/** Momentos (s) en que cada pieza encaja: retraso de la animación + ~0,35 s del rebote. */
export const PIECE_LANDINGS = [0.4, 0.47, 0.57, 0.71, 0.77] as const;
/** Momento en que se enciende el núcleo. */
export const CORE_AT = 0.92;
/** Duración total aproximada del sonido (s). */
export const INTRO_SOUND_SECONDS = 3.4;

function noiseBuffer(ctx: BaseAudioContext, seconds: number): AudioBuffer {
  const buf = ctx.createBuffer(1, Math.max(1, Math.floor(ctx.sampleRate * seconds)), ctx.sampleRate);
  const d = buf.getChannelData(0);
  // Generador determinista: el sonido es siempre el mismo.
  let x = 0x9e3779b9;
  for (let i = 0; i < d.length; i++) {
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    d[i] = ((x >>> 0) / 4294967295) * 2 - 1;
  }
  return buf;
}

function reverbImpulse(ctx: BaseAudioContext, seconds: number): AudioBuffer {
  const len = Math.floor(ctx.sampleRate * seconds);
  const buf = ctx.createBuffer(2, len, ctx.sampleRate);
  for (let ch = 0; ch < 2; ch++) {
    const d = buf.getChannelData(ch);
    let x = 0x2545f491 + ch * 7919;
    for (let i = 0; i < len; i++) {
      x ^= x << 13;
      x ^= x >>> 17;
      x ^= x << 5;
      const n = ((x >>> 0) / 4294967295) * 2 - 1;
      d[i] = n * Math.pow(1 - i / len, 2.6);
    }
  }
  return buf;
}

/** Programa el sonido completo en `ctx` empezando en `t0`. Devuelve la salida principal. */
export function scheduleIntroSound(ctx: BaseAudioContext, t0: number, volume = 0.6): GainNode {
  const master = ctx.createGain();
  master.gain.value = volume;
  const comp = ctx.createDynamicsCompressor();
  comp.threshold.value = -16;
  comp.ratio.value = 4;
  master.connect(comp).connect(ctx.destination);

  const reverb = ctx.createConvolver();
  reverb.buffer = reverbImpulse(ctx, 1.8);
  const wet = ctx.createGain();
  wet.gain.value = 0.28;
  reverb.connect(wet).connect(master);

  const out = (node: AudioNode, reverbSend = 0.5) => {
    node.connect(master);
    if (reverbSend > 0) {
      const s = ctx.createGain();
      s.gain.value = reverbSend;
      node.connect(s).connect(reverb);
    }
  };

  const noise = noiseBuffer(ctx, 1.2);

  // 1) Whoosh: ruido con un filtro que barre de grave a agudo mientras vuelan las piezas.
  {
    const src = ctx.createBufferSource();
    src.buffer = noise;
    const bp = ctx.createBiquadFilter();
    bp.type = "bandpass";
    bp.Q.value = 1.4;
    bp.frequency.setValueAtTime(260, t0);
    bp.frequency.exponentialRampToValueAtTime(2600, t0 + 0.55);
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t0);
    g.gain.exponentialRampToValueAtTime(0.5, t0 + 0.32);
    g.gain.exponentialRampToValueAtTime(0.0001, t0 + 0.7);
    src.connect(bp).connect(g);
    out(g, 0.3);
    src.start(t0);
    src.stop(t0 + 0.75);
  }

  // 2) Golpe de bloque al encajar cada pieza (cada uno un poco más agudo).
  PIECE_LANDINGS.forEach((at, i) => {
    const t = t0 + at;
    const f = 150 * Math.pow(1.13, i);
    const osc = ctx.createOscillator();
    osc.type = "triangle";
    osc.frequency.setValueAtTime(f * 1.9, t);
    osc.frequency.exponentialRampToValueAtTime(f * 0.55, t + 0.11);
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(0.42, t + 0.006);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 0.16);
    osc.connect(g);
    out(g, 0.25);
    osc.start(t);
    osc.stop(t + 0.18);

    const click = ctx.createBufferSource();
    click.buffer = noise;
    const hp = ctx.createBiquadFilter();
    hp.type = "bandpass";
    hp.frequency.value = 2200 + i * 260;
    hp.Q.value = 2.2;
    const cg = ctx.createGain();
    cg.gain.setValueAtTime(0.0001, t);
    cg.gain.exponentialRampToValueAtTime(0.3, t + 0.003);
    cg.gain.exponentialRampToValueAtTime(0.0001, t + 0.045);
    click.connect(hp).connect(cg);
    out(cg, 0.15);
    click.start(t, 0.1 * i, 0.06);
  });

  // 3) Encendido del núcleo: un barrido corto hacia arriba…
  {
    const t = t0 + CORE_AT - 0.1;
    const osc = ctx.createOscillator();
    osc.type = "sine";
    osc.frequency.setValueAtTime(420, t);
    osc.frequency.exponentialRampToValueAtTime(1320, t + 0.12);
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(0.16, t + 0.06);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 0.16);
    osc.connect(g);
    out(g, 0.4);
    osc.start(t);
    osc.stop(t + 0.18);
  }

  // …y un acorde brillante tipo campana (La mayor con novena) con bajo suave.
  {
    const t = t0 + CORE_AT;
    const notes = [440, 554.37, 659.25, 987.77];
    notes.forEach((f, i) => {
      const start = t + i * 0.035;
      for (const [ratio, level, decay] of [
        [1, 0.16, 2.2],
        [2.76, 0.035, 0.7],
        [5.4, 0.012, 0.35],
      ] as const) {
        const osc = ctx.createOscillator();
        osc.type = "sine";
        osc.frequency.value = f * ratio;
        const g = ctx.createGain();
        g.gain.setValueAtTime(0.0001, start);
        g.gain.exponentialRampToValueAtTime(level, start + 0.012);
        g.gain.exponentialRampToValueAtTime(0.0001, start + decay);
        osc.connect(g);
        out(g, 0.9);
        osc.start(start);
        osc.stop(start + decay + 0.05);
      }
    });
    const sub = ctx.createOscillator();
    sub.type = "sine";
    sub.frequency.setValueAtTime(110, t);
    sub.frequency.exponentialRampToValueAtTime(55, t + 0.5);
    const sg = ctx.createGain();
    sg.gain.setValueAtTime(0.0001, t);
    sg.gain.exponentialRampToValueAtTime(0.32, t + 0.02);
    sg.gain.exponentialRampToValueAtTime(0.0001, t + 0.7);
    sub.connect(sg);
    out(sg, 0);
    sub.start(t);
    sub.stop(t + 0.75);
  }

  // 4) Chispas suaves mientras aparecen las letras de "MercHosting".
  for (let i = 0; i < 11; i += 2) {
    const t = t0 + 0.85 + i * 0.045;
    const osc = ctx.createOscillator();
    osc.type = "sine";
    osc.frequency.value = 2093 * Math.pow(2, i / 24);
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(0.03, t + 0.005);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 0.09);
    osc.connect(g);
    out(g, 0.8);
    osc.start(t);
    osc.stop(t + 0.1);
  }

  return master;
}

const STORAGE_KEY = "merchosting.introSound";

export function introSoundEnabled(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) !== "off";
  } catch {
    return true;
  }
}

export function setIntroSoundEnabled(on: boolean) {
  try {
    localStorage.setItem(STORAGE_KEY, on ? "on" : "off");
  } catch {
    // Sin almacenamiento disponible: la preferencia dura solo esta sesión.
  }
}

/**
 * Reproduce el sonido de entrada si está activado. Si el sistema no permite reproducir audio
 * sin interacción, no suena (no se fuerza nada).
 */
let playedThisSession = false;

export function playIntroSound(): void {
  // En desarrollo React monta dos veces los componentes: el sonido solo debe sonar una vez.
  if (playedThisSession || !introSoundEnabled()) return;
  playedThisSession = true;
  const Ctx = (window as unknown as { AudioContext?: typeof AudioContext }).AudioContext;
  if (!Ctx) return;
  try {
    const ctx = new Ctx();
    const go = () => {
      scheduleIntroSound(ctx, ctx.currentTime + 0.03);
      setTimeout(() => ctx.close().catch(() => undefined), (INTRO_SOUND_SECONDS + 0.5) * 1000);
    };
    if (ctx.state === "suspended") {
      ctx.resume().then(go, () => ctx.close().catch(() => undefined));
    } else {
      go();
    }
  } catch {
    // Audio no disponible en este equipo.
  }
}
