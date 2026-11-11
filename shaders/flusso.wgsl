// nome: Flusso
// Un fiume di corsie inclinate a 50°, come la barra "/" del marchio, percorse da pacchetti di luce.

fn logo_layout() -> vec3f {
    let w = min(0.26 * u.canvas.x, 0.46 * u.canvas.y) / 0.9;
    return vec3f(0.09 * u.canvas.x + w * 0.5, 0.5 * u.canvas.y, w);
}

// hash economico: sequenza di Weyl rimescolata (niente seni)
fn fh4(c: vec2f) -> vec4f {
    let b = fract(dot(c, vec2f(0.7548776662, 0.5698402910)) + 0.137);
    return fract(b * vec4f(97.31, 211.73, 389.17, 607.91) + b * b * vec4f(13.7, 29.3, 7.1, 51.9));
}

// "seno" economico: onda parabolica di periodo 1, valori in -1..1
fn fsin(x: f32) -> f32 { let f = fract(x) * 2.0 - 1.0; return 4.0 * f * (1.0 - abs(f)); }

// Colore della corsia secondo la posizione nel fiume (0 = riva alta, 1 = riva bassa):
// ciano → azzurro → indaco → viola
fn lane_col(x: f32) -> vec3f {
    if (x < 0.35) { return mix(rgb(86.0, 214.0, 246.0), rgb(49.0, 140.0, 211.0), x / 0.35); }
    if (x < 0.65) { return mix(rgb(49.0, 140.0, 211.0), rgb(90.0, 100.0, 240.0), (x - 0.35) / 0.3); }
    return mix(rgb(90.0, 100.0, 240.0), rgb(170.0, 90.0, 230.0), sat((x - 0.65) / 0.35));
}

// Una corsia: filo sottile + pacchetti di luce che la percorrono.
// y = distanza dal centro della corsia, m = coordinata lungo la corsia, id = indice della corsia,
// S = passo tra le corsie, sc = scala della velocità, c = colore, env = intensità della corsia.
fn lane(y: f32, l: f32, id: f32, S: f32, sc: f32, c: vec3f, env: f32, layer: f32) -> vec3f {
    let k = kpx();
    let h = fh4(vec2f(id, layer * 37.0));
    if (h.x > 0.8 || env <= 0.0) { return vec3f(0.0); }
    let ay = abs(y);
    // l'alone si spegne prima del bordo della corsia: nessun salto tra una corsia e l'altra
    let edge = sat(1.0 - ay / (0.5 * S));
    var o = c * (0.08 + 0.10 * h.w) * (stroke(ay, 0.8) + 0.18 * edge * edge);
    // pacchetti
    let v = (36.0 + 46.0 * h.y) * k * sc;
    let P = (420.0 + 520.0 * h.z) * k;
    let m = l - u.time * v + h.w * 5000.0 * k;
    let cell = floor(m / P);
    let hh = fh4(vec2f(id * 1.37 + layer * 91.0, cell));
    if (hh.x < 0.6) {
        let len = (90.0 + 190.0 * hh.y) * k;
        let head = (cell + 0.05 + hh.z * 0.6) * P + len;
        let x = head - m;                      // distanza dietro la testa
        if (x > -14.0 * k && x < len) {
            let f = sat(1.0 - x / len);
            let tail = select(f * f, exp(-x * x / (40.0 * k * k)), x < 0.0);
            let hot = mix(c, vec3f(1.0), 0.55 * tail);
            o += hot * tail * (0.3 + 0.7 * hh.w) * (stroke(ay, 1.5) + 0.4 * edge * edge * tail);
        }
    }
    return o * env;
}

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let C = u.canvas * 0.5;
    let vm = max(W, H);
    let open = ease_out((u.since - 0.1) / 2.6);

    // ---- sfondo: inchiostro che schiarisce verso il blu notte in basso a destra ----
    let g = sat(dot(p / u.canvas, vec2f(0.55, 0.45)));
    var col = mix(rgb(5.0, 4.0, 16.0), rgb(10.0, 14.0, 42.0), g);
    let n1 = vec2f(W * (0.8 + 0.04 * fsin(t * 0.004)), H * (0.25 + 0.05 * fsin(t * 0.0033 + 0.25)));
    col = mix(col, rgb(40.0, 48.0, 170.0), 0.16 * sat(1.0 - length(p - n1) / (0.5 * vm)));
    let n2 = vec2f(W * (0.62 + 0.04 * fsin(t * 0.0029 + 0.5)), H * (0.95 + 0.04 * fsin(t * 0.0041)));
    col = mix(col, rgb(120.0, 24.0, 170.0), 0.14 * sat(1.0 - length(p - n2) / (0.45 * vm)));

    // ---- riferimento inclinato: l lungo la "/", a di traverso ----
    let d = vec2f(0.6427876, -0.7660444);
    let nn = vec2f(0.7660444, 0.6427876);
    let q = p - C;
    let l = dot(q, d);
    let D = length(u.canvas) * 0.5;
    let a = dot(q, nn);

    // il fiume serpeggia lentamente e respira in larghezza
    let ac = 0.13 * W + 0.10 * H * fsin(l / (1.9 * H) + t * 0.0035);
    let hw = 0.19 * H * (1.0 + 0.22 * fsin(l / (1.3 * H) - t * 0.0027 + 0.4));
    let ar = a - ac;
    let xr = ar / hw;                 // -1..1 dentro il fiume

    // comparsa: le corsie si allungano dal basso a sinistra verso l'alto a destra
    let front = mix(-1.1 * D, 1.2 * D, open);
    let grow = sat((front - l) / (160.0 * k));

    // bagliore diffuso del fiume
    let rc = mix(rgb(49.0, 120.0, 211.0), rgb(120.0, 40.0, 190.0), sat(xr * 0.5 + 0.5));
    col += rc * 0.13 / (1.0 + xr * xr * 1.4) * grow;
    // le due rive: fili al neon con un riflesso che le percorre piano
    let bank = min(abs(ar - hw), abs(ar + hw));
    if (bank < 30.0 * k) {
        let up = ar < 0.0;
        let bc = select(rgb(194.0, 136.0, 214.0), rgb(112.0, 189.0, 232.0), up);
        let sw = fract(t * 0.021 + select(0.5, 0.0, up)) * 2.6 - 1.3;
        let sh = exp(-(l / D - sw) * (l / D - sw) * 9.0);
        let bx = bank / k;
        col += mix(bc, vec3f(1.0), 0.4 * sh) * (0.22 + 0.6 * sh) * (stroke(bank, 1.1) + 0.12 / (1.0 + bx * bx * 0.08)) * grow;
    }

    // ---- corsie del fiume (una sola per pixel) ----
    let S = 15.0 * k;
    let id = floor(ar / S);
    let y = ar - (id + 0.5) * S;
    let xc = ((id + 0.5) * S) / hw;
    let x2 = xc * xc;
    let env = sat(1.0 - x2 * x2) * (0.75 + 0.25 * sat(1.0 - x2)) * grow;
    col += lane(y, l, id, S, 1.0, lane_col(sat(xc * 0.5 + 0.5)), env, 0.0);

    // ---- corsie lontane, rade e lente, su tutto lo schermo (profondità) ----
    let S2 = 58.0 * k;
    let a2 = a + 0.37 * S2;
    let id2 = floor(a2 / S2);
    let y2 = a2 - (id2 + 0.5) * S2;
    let env2 = 0.3 * sat(0.3 + xr * xr * 0.2) * grow;
    col += lane(y2, l * 1.0, id2, S2, 0.55, mix(rgb(112.0, 189.0, 232.0), rgb(194.0, 136.0, 214.0), sat(p.x / W)), env2, 1.0);

    // lato sinistro più scuro, dove sta il logo
    let lo = logo_layout();
    let dl = length((p - lo.xy) / vec2f(lo.z * 0.95, lo.z * 0.6));
    col = mix(col, rgb(4.0, 3.0, 12.0), 0.7 * sat(1.0 - dl * dl * 0.6) * u.logo_on);
    let xl = p.x / (0.5 * W);
    col = mix(col, rgb(4.0, 3.0, 12.0), 0.35 * sat(1.0 - xl));
    // vignettatura
    let dv = length(p - C);
    let r0 = min(W, H) * 0.42;
    let r1 = length(u.canvas) * 0.62;
    col *= 1.0 - 0.5 * sat((dv - r0) / (r1 - r0));
    return col;
}
