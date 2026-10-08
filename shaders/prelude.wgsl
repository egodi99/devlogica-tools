// ============================================================================
//  Preludio comune a tutti gli sfondi DevLogica.
//  Ogni sfondo deve definire:
//     fn scene(p: vec2f) -> vec3f      colore del pixel p (coordinate della tela, in pixel, y verso il basso)
//     fn logo_layout() -> vec3f        (centro x, centro y, larghezza) del logo, in pixel della tela
//  Tutto il resto (vertici, logo, retinatura anti-banding) è gestito qui.
// ============================================================================

struct Params {
    res: vec2f,      // dimensione di questa finestra
    offset: vec2f,   // posizione della finestra nella tela (modalità estesa)
    canvas: vec2f,   // dimensione della tela
    time: f32,       // secondi, condivisi da tutte le finestre
    since: f32,      // secondi dall'ultimo cambio di sfondo
    logo_on: f32,
    scale: f32,      // pixel disegnati / pixel della finestra
    _p1: f32, _p2: f32,
};

@group(0) @binding(0) var<uniform> u: Params;
@group(0) @binding(1) var logo_tex: texture_2d<f32>;
@group(0) @binding(2) var logo_smp: sampler;

const PI: f32 = 3.14159265;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    return vec4f(x, y, 0.0, 1.0);
}

// ---------- utilità ----------
fn rgb(r: f32, g: f32, b: f32) -> vec3f { return vec3f(r, g, b) / 255.0; }
fn sat(x: f32) -> f32 { return clamp(x, 0.0, 1.0); }
// coverage di un tratto di larghezza w (in pixel di riferimento)
fn stroke(d: f32, w: f32) -> f32 { let k = kpx(); return 1.0 - smoothstep(w * 0.5 * k - 0.5, w * 0.5 * k + 0.5, d); }
fn ease_out(x: f32) -> f32 { let c = sat(x); return 1.0 - (1.0 - c) * (1.0 - c) * (1.0 - c); }
fn hash11(x: f32) -> f32 { return fract(sin(x * 127.1 + 311.7) * 43758.5453); }
fn hash21(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn rot2(a: f32) -> mat2x2f { let c = cos(a); let s = sin(a); return mat2x2f(c, s, -s, c); }
// unità "pixel di riferimento": 1 a 1080 righe, così gli spessori scalano con lo schermo
fn kpx() -> f32 { return u.canvas.y / 1080.0; }
// Distanza dal segmento ab; .y = posizione lungo il segmento (0..1).
fn seg(p: vec2f, a: vec2f, b: vec2f) -> vec2f {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return vec2f(length(pa - ba * h), h);
}
// Tratto "al neon": nucleo nitido + due aloni, come le linee dei canvas originali.
fn neon(d: f32, w: f32) -> f32 {
    let k = kpx();
    let core = 1.0 - smoothstep(w * 0.5 * k, w * 0.5 * k + k, d);
    return core + 0.18 * exp(-d / (2.0 * k)) + 0.08 * exp(-d / (6.0 * k));
}
// Bagliore radiale morbido.
fn glow(d: f32, r: f32) -> f32 { return exp(-(d * d) / (r * r)); }

// ---------- logo ----------
fn logo_rect() -> vec4f {
    let l = logo_layout();
    let dims = vec2f(textureDimensions(logo_tex));
    let h = l.z * dims.y / dims.x;
    return vec4f(l.x - l.z * 0.5, l.y - h * 0.5, l.z, h);
}

fn apply_logo(p: vec2f, col: vec3f) -> vec3f {
    let r = logo_rect();
    let uv = (p - r.xy) / r.zw;
    let s = textureSample(logo_tex, logo_smp, uv); // premoltiplicato
    let inside = step(0.0, uv.x) * step(uv.x, 1.0) * step(0.0, uv.y) * step(uv.y, 1.0);
    // comparsa dopo l'animazione d'apertura
    let a = smoothstep(1.3, 2.6, u.since) * inside * u.logo_on;
    var c = col * (1.0 - s.a * a) + s.rgb * a;
    // riflesso diagonale che attraversa il logo ogni 12 secondi
    let ph = fract(u.time / 12.0) * 1.6 - 0.3;
    let band = uv.x + (uv.y - 0.5) * 0.35 - ph * 1.4;
    c += vec3f(1.0) * s.a * a * 0.45 * exp(-band * band * 180.0);
    return c;
}

@fragment
fn fs_main(@builtin(position) fc: vec4f) -> @location(0) vec4f {
    let p = fc.xy / u.scale + u.offset;
    var col = scene(p);
    col = apply_logo(p, col);
    // retinatura leggera: elimina le "scalette" nei gradienti scuri
    col += (hash21(p + fract(u.time) * 17.0) - 0.5) / 255.0 * 1.5;
    return vec4f(clamp(col, vec3f(0.0), vec3f(1.0)), 1.0);
}
