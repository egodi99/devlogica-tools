// nome: Costellazione
// Rete di nodi luminosi che derivano piano, legati da fili sottili su cui corrono impulsi di luce.

fn logo_layout() -> vec3f {
    let w = min(0.30 * u.canvas.x, 0.533 * u.canvas.y) / 0.9;
    return vec3f(0.5 * u.canvas.x, 0.5 * u.canvas.y, w);
}

// I nodi stanno su un reticolo triangolare (assi a 0° e a 50°, come la barra "/" del marchio),
// spostati a caso e in lenta deriva. Ogni pixel cade in un solo triangolo: bastano i suoi
// 3 nodi e i suoi 3 lati, che sono condivisi con i triangoli accanto (niente fili tagliati).

// hash molto economico: sequenza di Weyl sulla cella, poi rimescolata con moltiplicatori diversi
fn qh4(c: vec2f) -> vec4f {
    let b = fract(dot(c, vec2f(0.7548776662, 0.5698402910)) + 0.137);
    return fract(b * vec4f(97.31, 211.73, 389.17, 607.91) + b * b * vec4f(13.7, 29.3, 7.1, 51.9));
}

// "seno" economico: onda parabolica di periodo 1, valori in -1..1 (nessuna trascendente)
fn psin(x: f32) -> f32 { let f = fract(x) * 2.0 - 1.0; return 4.0 * f * (1.0 - abs(f)); }

// passo del reticolo (pixel della tela)
fn cell_size() -> f32 { return clamp(u.canvas.x / 12.0, 130.0 * kpx(), 200.0 * kpx()); }
// secondo asse del reticolo: 50° verso l'alto a destra (y verso il basso)
const BX: f32 = 0.6427876;
const BY: f32 = -0.7660444;

// Comparsa: un'onda che parte dal logo e si allarga verso i bordi.
fn reveal(q: vec2f) -> f32 {
    let C = u.canvas * 0.5;
    let R = ease_out((u.since - 0.15) / 2.8) * length(u.canvas) * 0.62;
    return sat((R - length(q - C)) / (220.0 * kpx()));
}

// Nodo (I, J): .xy posizione, .z luminosità (0 = assente), .w seme per fili e respiro
fn node(c: vec2f) -> vec4f {
    let G = cell_size();
    let h = qh4(c);
    // deriva lenta su una piccola ellisse attorno a un punto fisso a caso
    let a = u.time * (0.008 + 0.008 * h.z) + h.x;
    let off = vec2f(psin(a + 0.25), psin(a * 0.8 + h.w)) * 0.03 + (h.xy - 0.5) * 0.07;
    let lat = vec2f(c.x + c.y * BX, c.y * BY) + off;
    let alive = select(0.0, 0.45 + 0.55 * h.w, h.z < 0.8);
    return vec4f(lat * G, alive, h.y);
}

// Filo da a a b. seed/ty identificano il lato in modo stabile (lo stesso per i due triangoli che lo condividono).
fn link(p: vec2f, a: vec4f, b: vec4f, seed: f32, ty: f32) -> vec3f {
    let h = fract(seed * vec2f(17.13, 31.71) * (ty + 1.0) + ty * 0.37);
    if (h.x > 0.5 || a.z * b.z <= 0.0) { return vec3f(0.0); }
    let k = kpx();
    let s = seg(p, a.xy, b.xy);
    if (s.x > 12.0 * k) { return vec3f(0.0); }
    let base = mix(rgb(194.0, 136.0, 214.0), rgb(112.0, 189.0, 232.0), sat(0.5 * (a.x + b.x) / u.canvas.x));
    let x = s.x / k;
    // il filo si stacca dai nodi: lascia un piccolo vuoto attorno a ogni stella
    let L = length(b.xy - a.xy);
    let gap = sat((min(s.y, 1.0 - s.y) * L - 0.09 * cell_size()) / (6.0 * k));
    if (gap <= 0.0) { return vec3f(0.0); }
    // filo: nucleo sottile e appena un alone
    var o = base * (0.10 + 0.06 * h.y) * (stroke(s.x, 0.9) + 0.25 / (1.0 + x * x * 0.15));
    // impulso: solo su una parte dei fili, a cicli lenti e sfasati
    if (h.y < 0.36) {
        let ph = fract(u.time / (7.0 + 9.0 * h.y) + h.x * 1.9);
        let pos = ph * 2.2 - 0.6;               // gran parte del ciclo è fuori dal filo: impulsi radi
        let dir = select(s.y, 1.0 - s.y, fract(h.x * 7.0) < 0.5);
        let back = pos - dir;
        if (back > -0.08 && back < 0.45) {
            let z = back * L / k;
            let fb = sat(1.0 - back / 0.45);
            let tail = select(fb * fb / (1.0 + z / 40.0), 1.0 / (1.0 + z * z / 16.0), back < 0.0);
            let c = mix(base, vec3f(1.0), 0.45 * tail);
            o += c * tail * (stroke(s.x, 1.6) * 0.9 + 0.35 / (1.0 + x * x * 0.1));
        }
    }
    return o * gap;
}

// Punto luminoso del nodo, con un respiro lento.
fn star(p: vec2f, n: vec4f) -> vec3f {
    let k = kpx();
    let d = length(p - n.xy);
    if (n.z <= 0.0 || d > 34.0 * k) { return vec3f(0.0); }
    let br = n.z * (0.7 + 0.3 * psin(u.time * (0.05 + 0.06 * fract(n.w * 7.3)) + n.w * 6.0));
    let base = mix(rgb(200.0, 160.0, 235.0), rgb(150.0, 210.0, 245.0), sat(n.x / u.canvas.x));
    let r = (1.4 + 1.4 * fract(n.w * 3.1)) * k;
    let x = d / k;
    let fo = sat(1.0 - x / 34.0);
    return mix(base, vec3f(1.0), 0.5) * br * (1.0 - smoothstep(r - 0.5, r + 0.5, d))
         + base * br * (0.22 / (1.0 + x * x / 20.0) + 0.06 * fo * fo) * fo;
}

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let C = u.canvas * 0.5;
    let vm = max(W, H);

    // ---- sfondo: blu notte con due nubi di colore che si spostano piano ----
    let s = sat(p.x / W * 0.7 + p.y / H * 0.3);
    var col = mix(rgb(16.0, 9.0, 40.0), rgb(8.0, 16.0, 40.0), s);
    let n1 = vec2f(W * (0.24 + 0.04 * psin(t * 0.005)), H * (0.32 + 0.05 * psin(t * 0.0043 + 0.25)));
    col = mix(col, rgb(110.0, 20.0, 160.0), 0.17 * sat(1.0 - length(p - n1) / (0.42 * vm)));
    let n2 = vec2f(W * (0.78 + 0.04 * psin(t * 0.0037 + 0.25)), H * (0.70 + 0.05 * psin(t * 0.0056)));
    col = mix(col, rgb(40.0, 120.0, 200.0), 0.13 * sat(1.0 - length(p - n2) / (0.40 * vm)));

    let rv = reveal(p);

    // ---- polvere di stelle minuscole (una cella per pixel) ----
    let g2 = 46.0 * k;
    let c2 = floor(p / g2);
    let h2 = qh4(c2 + 300.0);
    if (h2.x < 0.35) {
        let q = (c2 + 0.2 + 0.6 * h2.zw) * g2;
        let tw = 0.5 + 0.5 * psin(t * (0.06 + 0.1 * h2.y) + h2.x * 3.0);
        let dq = length(p - q) / (1.3 * k);
        col += rgb(170.0, 170.0, 240.0) * 0.22 * tw * sat(1.0 - dq * dq * 0.25) * rv;
    }

    // ---- costellazione: il triangolo del reticolo che contiene il pixel ----
    let G = cell_size();
    // deformazione dolce e lentissima dello spazio: il reticolo perde la regolarità
    // senza spezzare i fili (tutto è calcolato nello spazio deformato)
    let pw = p + vec2f(psin(p.y / (4.3 * G) + t * 0.0021), psin(p.x / (5.1 * G) - t * 0.0017 + 0.3)) * 0.22 * G;
    let jf = pw.y / (G * BY);
    let lc = vec2f(pw.x / G - jf * BX, jf);
    let c = floor(lc);
    let f = lc - c;
    let up = f.x + f.y > 1.0;
    let nA = node(c + vec2f(1.0, 0.0));
    let nB = node(c + vec2f(0.0, 1.0));
    let nC = node(select(c, c + vec2f(1.0, 1.0), up));
    var net = vec3f(0.0);
    // lato obliquo comune ai due triangoli della cella
    net += link(pw, nA, nB, nA.w, 2.0);
    if (up) {
        net += link(pw, nB, nC, nB.w, 0.0);   // asse orizzontale, dal nodo (I, J+1)
        net += link(pw, nA, nC, nA.w, 1.0);   // asse a 50°, dal nodo (I+1, J)
    } else {
        net += link(pw, nC, nA, nC.w, 0.0);
        net += link(pw, nC, nB, nC.w, 1.0);
    }
    // stelle: basta la più vicina delle tre (i nodi distano molto più del raggio dell'alone)
    let dA = dot(pw - nA.xy, pw - nA.xy);
    let dB = dot(pw - nB.xy, pw - nB.xy);
    let dC = dot(pw - nC.xy, pw - nC.xy);
    let nAB = select(nB, nA, dA < dB);
    net += star(pw, select(nC, nAB, min(dA, dB) < dC));
    net *= rv;

    // la rete si dirada attorno al logo
    let e = length((p - C) / vec2f(0.30 * W, 0.17 * W));
    let quiet = mix(1.0, smoothstep(0.35, 1.05, e), u.logo_on);
    col += net * mix(0.35, 1.0, quiet);

    // alone scuro dietro al logo
    let ha = select(select(0.0, mix(0.45, 0.0, (e - 0.55) / 0.45), e < 1.0), mix(0.8, 0.45, e / 0.55), e < 0.55);
    col = mix(col, rgb(12.0, 8.0, 30.0), ha * u.logo_on);
    // vignettatura
    let dv = length((p - C) / vec2f(1.2 * vm, 0.8 * vm));
    col = mix(col, rgb(3.0, 2.0, 10.0), 0.55 * sat((dv - 0.45) / 0.55));
    return col;
}
