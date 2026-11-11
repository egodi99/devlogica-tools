// nome: Isoipse
// tipo: fisso
// Carta topografica notturna: curve di livello sottili dal viola al ciano, con le quote maestre più marcate.

fn logo_layout() -> vec3f {
    let w = min(0.30 * u.canvas.x, 0.533 * u.canvas.y) / 0.9;
    return vec3f(0.5 * u.canvas.x, 0.5 * u.canvas.y, w);
}

// rumore a valori, interpolazione quintica
fn is_noise(x: vec2f) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let w = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let a = hash21(i);
    let b = hash21(i + vec2f(1.0, 0.0));
    let c = hash21(i + vec2f(0.0, 1.0));
    let d = hash21(i + vec2f(1.0, 1.0));
    return mix(mix(a, b, w.x), mix(c, d, w.x), w.y);
}

// "terreno": somma di ottave ruotate più due rilievi principali che danno la composizione
fn is_height(q: vec2f) -> f32 {
    var h = 0.0;
    var amp = 0.5;
    var x = q * 2.2 + vec2f(3.1, 7.4);
    let m = mat2x2f(0.8, 0.6, -0.6, 0.8) * 2.03;
    for (var o = 0; o < 4; o++) {
        h += amp * is_noise(x);
        x = m * x;
        amp *= 0.42;
    }
    let ar = u.canvas.x / u.canvas.y;
    // rilievo grande a destra, allungato lungo la diagonale "/"
    let r1 = rot2(0.87) * (q - vec2f(ar * 0.80, 0.52));
    h += 0.75 * exp(-dot(r1 / vec2f(0.42, 0.24), r1 / vec2f(0.42, 0.24)));
    // rilievo minore in basso a sinistra
    let r2 = q - vec2f(ar * 0.16, 0.92);
    h += 0.45 * exp(-dot(r2, r2) / 0.07);
    return h;
}

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let C = u.canvas * 0.5;
    let vm = max(W, H);

    let SKY = rgb(112.0,189.0,232.0);
    let CY = rgb(86.0,214.0,246.0);
    let IND = rgb(90.0,100.0,240.0);
    let VI = rgb(134.0,18.0,173.0);
    let LI = rgb(194.0,136.0,214.0);
    let MAG = rgb(200.0,110.0,240.0);

    // ---- quota e pendenza (differenze finite di un pixel) ----
    let q = p / H;
    let e = 1.0 / H;
    let h = is_height(q);
    let hx = is_height(q + vec2f(e, 0.0));
    let hy = is_height(q + vec2f(0.0, e));
    let gpx = length(vec2f(hx - h, hy - h));   // variazione di quota per pixel

    // ---- sfondo: blu notte, più caldo dove il terreno è basso ----
    let hn = sat((h - 0.35) / 0.9);
    var col = mix(rgb(10.0, 6.0, 26.0), rgb(8.0, 14.0, 38.0), hn);
    col = mix(col, rgb(70.0, 20.0, 120.0), 0.22 * glow(length(p - vec2f(0.12 * W, 0.95 * H)), 0.5 * vm));
    col = mix(col, rgb(28.0, 70.0, 150.0), 0.20 * glow(length(p - vec2f(0.80 * W, 0.50 * H)), 0.45 * vm));
    // ombreggiatura del rilievo, appena accennata (luce da nord-ovest)
    let nrm = normalize(vec3f(-(hx - h) * 70.0, -(hy - h) * 70.0, e));
    let sh = dot(nrm, normalize(vec3f(-0.6, -0.7, 0.4)));
    col *= 0.9 + 0.18 * sat(sh);

    // ---- curve di livello ----
    let N = 26.0;                       // curve per unità di quota
    let lv = h * N;
    let dl = abs(fract(lv + 0.5) - 0.5) / max(gpx * N, 1e-5);   // distanza in pixel dalla curva
    let idx = floor(lv + 0.5);
    let major = abs(idx - 5.0 * round(idx / 5.0)) < 0.5;
    // colore per quota: viola in basso, indaco, ciano in cima
    let lc = select(mix(IND, CY, (hn - 0.5) / 0.5), mix(VI * 1.3, IND, hn / 0.5), hn < 0.5);
    // dove le curve si addensano troppo, le attenuo per non creare grumi
    let dens = sat(1.0 - (gpx * N * 3.0 * k - 0.35) / 0.5);
    var line = 0.0;
    if (major) {
        line = (stroke(dl, 1.3) * 0.75 + 0.10 * exp(-dl / (4.0 * k))) * mix(0.55, 1.0, dens);
    } else {
        line = stroke(dl, 0.8) * 0.28 * dens;
    }
    // più tenui sul bordo sinistro e in alto a destra, dove stanno le icone
    let calm = 1.0 - 0.35 * sat(1.0 - p.x / (0.22 * W)) - 0.3 * sat((p.x / W - 0.75) / 0.25) * sat(1.0 - p.y / (0.35 * H));
    col += mix(lc, vec3f(1.0), 0.1 * hn) * line * calm;

    // ---- bagliore sul rilievo principale ----
    col += CY * 0.07 * glow(length(p - vec2f(0.80 * W, 0.56 * H)), 0.25 * H);

    // ---- reticolo cartografico: crocette discrete ogni tanto ----
    let G = 0.16 * H;
    let gc = (p - C) / G;
    let gd = abs(gc - round(gc)) * G;
    let cross = select(0.0, 1.0, max(gd.x, gd.y) < 6.0 * k) * max(stroke(gd.x, 0.8), stroke(gd.y, 0.8));
    col += LI * 0.10 * cross;

    // ---- alone scuro dietro al logo ----
    let el = length((p - C) / vec2f(0.31 * W, 0.17 * W));
    let ha = select(select(0.0, mix(0.5, 0.0, (el - 0.55) / 0.45), el < 1.0), mix(0.85, 0.5, el / 0.55), el < 0.55);
    col = mix(col, rgb(8.0, 6.0, 22.0), ha * u.logo_on);
    // vignettatura
    let dv = length((p - C) / vec2f(1.2 * vm, 0.8 * vm));
    col = mix(col, rgb(3.0, 2.0, 10.0), 0.6 * sat((dv - 0.4) / 0.6));
    return col;
}
