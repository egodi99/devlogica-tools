// nome: Aurora
// Tre nastri di aurora nei colori del marchio, con raggi inclinati come la "/" e un filo di luce sul bordo.

fn logo_layout() -> vec3f {
    let w = min(0.26 * u.canvas.x, 0.46 * u.canvas.y) / 0.9;
    return vec3f(0.5 * u.canvas.x, 0.7 * u.canvas.y, w);
}

// "seno" economico: onda parabolica di periodo 1, valori in -1..1 (nessuna trascendente)
fn asin1(x: f32) -> f32 { let f = fract(x) * 2.0 - 1.0; return 4.0 * f * (1.0 - abs(f)); }

// Un nastro. y0 = quota del bordo a sinistra (frazione di H), sl = pendenza, h = altezza della tenda,
// ph = fase, c1/c2 = colori del bordo e della cima, a = intensità.
fn ribbon(p: vec2f, y0: f32, sl: f32, h: f32, ph: f32, c1: vec3f, c2: vec3f, a: f32) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let x = p.x / H;
    // bordo inferiore: una curva lenta e ampia che ondeggia
    let yc = H * (y0 - sl * p.x / W
        + 0.085 * asin1(x * 0.19 + t * 0.007 + ph)
        + 0.022 * asin1(x * 0.61 - t * 0.0071 + ph * 1.7));
    let d = yc - p.y;                            // > 0 sopra il bordo
    if (d < -40.0 * k || d > h * H * 2.6) { return vec3f(0.0); }
    let hd = d / (h * H);
    // raggi inclinati come la barra "/": coordinata lungo il nastro spostata con l'altezza
    let s = (p.x - max(d, 0.0) * 0.84) / H;
    let r1 = 0.5 + 0.5 * asin1(s * 4.1 + t * 0.011 + ph);
    let r2 = 0.5 + 0.5 * asin1(s * 11.3 - t * 0.019 + ph * 3.1);
    let rays = 0.18 + 0.82 * r1 * r1 * (0.55 + 0.45 * r2);
    // intensità che pulsa piano lungo il nastro
    let br0 = 0.5 + 0.5 * asin1(x * 0.29 - t * 0.010 + ph * 0.7);
    let breath = 0.3 + 0.7 * br0;
    var o = vec3f(0.0);
    if (d > 0.0) {
        // tenda: forte vicino al bordo, sfuma verso l'alto
        let cur = (exp(-hd * 2.4) * 0.75 + 0.2 * exp(-hd * 0.8) + 0.5 * exp(-d / (22.0 * k))) * (1.0 - exp(-d / (6.0 * k)));
        o += mix(c1, c2, sat(hd * 0.9)) * cur * rays * breath * 1.25;
    }
    // filo luminoso sul bordo, con un riflesso che lo percorre
    let ad = abs(d);
    let sw = fract(t * 0.017 + ph * 0.31) * 1.8 - 0.4;
    let sh = exp(-(p.x / W - sw) * (p.x / W - sw) * 40.0);
    let z = ad / k;
    o += mix(c1, vec3f(1.0), 0.35 + 0.4 * sh) * (stroke(ad, 1.1) * (0.35 + 0.65 * sh) + 0.10 / (1.0 + z * z * 0.05)) * breath;
    // luce diffusa appena sotto il bordo
    if (d < 0.0) { o += c1 * 0.05 * exp(d / (14.0 * k)) * breath; }
    return o * a;
}

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let C = u.canvas * 0.5;
    let vm = max(W, H);
    let open = ease_out((u.since - 0.1) / 2.8);

    // ---- cielo: blu notte in alto, inchiostro in basso ----
    let gy = sat(p.y / H);
    var col = mix(rgb(12.0, 12.0, 40.0), rgb(4.0, 3.0, 12.0), gy);
    let n1 = vec2f(W * (0.3 + 0.05 * asin1(t * 0.003)), H * 0.15);
    col = mix(col, rgb(80.0, 20.0, 140.0), 0.16 * sat(1.0 - length((p - n1) / vec2f(0.6 * vm, 0.35 * vm))));

    // ---- nastri: compaiono uno dopo l'altro ----
    let r1 = sat(open * 1.6) ;
    let r2 = sat(open * 1.6 - 0.3);
    let r3 = sat(open * 1.6 - 0.6);
    // ogni nastro si "accende" scorrendo da sinistra verso destra
    let sx = p.x / W;
    col += ribbon(p, 0.46, 0.22, 0.20, 0.0, rgb(86.0, 214.0, 246.0), rgb(49.0, 140.0, 211.0), 0.9) * sat((r1 * 1.4 - sx) * 4.0);
    col += ribbon(p, 0.60, 0.26, 0.22, 0.37, rgb(90.0, 100.0, 240.0), rgb(134.0, 18.0, 173.0), 0.85) * sat((r2 * 1.4 - sx) * 4.0);
    col += ribbon(p, 0.33, 0.18, 0.15, 0.71, rgb(200.0, 110.0, 240.0), rgb(134.0, 18.0, 173.0), 0.7) * sat((r3 * 1.4 - sx) * 4.0);

    // ---- riflesso sull'orizzonte, in basso ----
    let hz = (p.y / H - 0.86) / 0.1;
    col += rgb(49.0, 90.0, 200.0) * 0.06 / (1.0 + hz * hz);

    // zona scura dietro al logo
    let lo = logo_layout();
    let dl = length((p - lo.xy) / vec2f(lo.z * 0.85, lo.z * 0.42));
    col = mix(col, rgb(4.0, 3.0, 12.0), 0.75 * sat(1.0 - dl * dl * 0.5) * u.logo_on);
    // vignettatura
    let dv = length((p - C) / vec2f(1.2 * vm, 0.8 * vm));
    col = mix(col, rgb(3.0, 2.0, 10.0), 0.55 * sat((dv - 0.45) / 0.55));
    return col;
}
