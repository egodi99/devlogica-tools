// nome: Cruscotto
// Plancia di un camion svedese anni '90, di notte. Lo shader disegna cabina, strada,
// plastica, radica, quadranti, display e volante; scale, lancette, spie e testi li
// disegna l'app sopra, con i valori veri del PC. GAUGES, ROCKERS e LCD_BOX arrivano
// da dash.rs (spazio di progetto 1920×1080, ancorato in basso al centro).

fn dk() -> f32 { return min(u.canvas.x / 1920.0, u.canvas.y / 1080.0); }
fn dorigin() -> vec2f { let k = dk(); return vec2f((u.canvas.x - 1920.0 * k) * 0.5, u.canvas.y - 1080.0 * k); }

fn logo_layout() -> vec3f {
    // targhetta sul bordo della plancia, sopra al quadro strumenti
    let k = dk();
    let c = dorigin() + vec2f(950.0, 506.0) * k;
    return vec3f(c.x, c.y, 150.0 * k);
}

// ---------- utilità ----------
fn sd_rbox(p: vec2f, c: vec2f, h: vec2f, r: f32) -> f32 {
    let q = abs(p - c) - h + vec2f(r);
    return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}
// copertura di "dentro" (d < 0) con bordo morbido di un pixel
fn inside(d: f32, px: f32) -> f32 { return 1.0 - smoothstep(-px, px, d); }

fn vnoise(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2f(1.0, 0.0));
    let c = hash21(i + vec2f(0.0, 1.0));
    let d = hash21(i + vec2f(1.0, 1.0));
    return mix(mix(a, b, w.x), mix(c, d, w.x), w.y);
}
fn fbm(p0: vec2f) -> f32 {
    var p = p0;
    var a = 0.5;
    var s = 0.0;
    for (var i = 0; i < 5; i++) {
        s += a * vnoise(p);
        p = rot2(0.6) * p * 2.03 + vec2f(1.7, 9.2);
        a *= 0.5;
    }
    return s;
}

// ---------- fuori: notte, strada ----------
const HZ: f32 = 300.0;
const VPX: f32 = 1010.0;

fn outside(d: vec2f, px: f32) -> vec3f {
    let t = u.time;
    // cielo: blu notte verso l'orizzonte, con il bagliore arancio di una città lontana
    let sky_t = sat((HZ - d.y) / 300.0);
    var col = mix(rgb(26.0, 22.0, 34.0), rgb(4.0, 6.0, 13.0), pow(sky_t, 0.6));
    col += rgb(90.0, 46.0, 22.0) * glow(length((d - vec2f(1260.0, HZ)) * vec2f(0.5, 2.2)), 220.0) * 0.55;
    col += rgb(40.0, 50.0, 90.0) * glow(length((d - vec2f(520.0, HZ)) * vec2f(0.6, 2.6)), 240.0) * 0.25;
    // stelle
    let cell = floor(d / 22.0);
    let h = hash21(cell);
    if (h > 0.92 && d.y < HZ - 30.0) {
        let sp = (cell + vec2f(hash21(cell + 3.1), hash21(cell + 7.7))) * 22.0;
        let tw = 0.6 + 0.4 * sin(t * (1.0 + h * 3.0) + h * 40.0);
        col += vec3f(0.55, 0.6, 0.75) * glow(length(d - sp), 1.3) * tw * (h - 0.92) * 9.0;
    }
    // colline
    let hill = HZ - 6.0 - 26.0 * fbm(vec2f(d.x * 0.0035, 3.0)) - 10.0 * fbm(vec2f(d.x * 0.012, 1.0));
    col = mix(col, rgb(7.0, 7.0, 11.0), inside(hill - d.y, px));
    // luci della città sull'orizzonte
    if (abs(d.y - (HZ - 4.0)) < 4.0 && d.x > 1100.0 && d.x < 1420.0) {
        let c2 = floor(d.x / 5.0);
        col += rgb(255.0, 180.0, 90.0) * step(0.8, hash11(c2)) * 0.35 * (0.7 + 0.3 * sin(t * 2.0 + c2));
    }
    if (d.y <= HZ) { return col; }

    // strada in prospettiva: x laterale costante lungo la corsia, z profondità
    let dy = max(d.y - HZ, 0.3);
    let lat = (d.x - VPX) / dy;          // unità di corsia ≈ 1.3
    let z = 120.0 / dy;                  // distanza
    let wz = z + u.dash.x * 40.0;        // posizione lungo la strada (si muove)
    let fw = fwidth(lat) * 1.2;
    var road = rgb(16.0, 16.0, 19.0) * (0.9 + 0.2 * vnoise(vec2f(lat * 6.0, wz * 3.0)));
    // fari: cono che illumina la strada vicina
    let beam = exp(-z * 0.16) * exp(-pow(lat - 0.2, 2.0) * 0.09);
    road += rgb(52.0, 50.0, 46.0) * beam;
    let on_road = sat((lat + 7.2) / fw) * sat((2.9 - lat) / fw);
    var shoulder = rgb(9.0, 11.0, 9.0) + rgb(26.0, 30.0, 18.0) * beam * 0.6 * (0.6 + 0.4 * fbm(vec2f(lat * 3.0, wz * 2.0)));
    col = mix(shoulder, road, on_road);
    // segnaletica
    let edge_r = 1.0 - smoothstep(0.06 - fw, 0.06 + fw, abs(lat - 2.55));
    let edge_l = 1.0 - smoothstep(0.06 - fw, 0.06 + fw, abs(lat + 6.85));
    let mid = (1.0 - smoothstep(0.07 - fw, 0.07 + fw, abs(lat + 2.15))) * step(fract(wz / 9.0), 0.42);
    let paint = max(max(edge_r, edge_l), mid);
    col = mix(col, rgb(190.0, 190.0, 180.0) * (0.25 + 1.6 * beam), paint * 0.9);
    // catarifrangenti sui paletti a destra
    let post = step(fract(wz / 22.0), 0.06) * (1.0 - smoothstep(0.08 - fw, 0.08 + fw, abs(lat - 3.4)));
    col += rgb(255.0, 140.0, 40.0) * post * sat(beam * 6.0 + 0.15) * 0.8;
    // un camion lontano davanti, nella nostra corsia: due luci rosse
    let far = vec2f(VPX + 0.5 * 10.0, HZ + 10.0);
    col += rgb(255.0, 30.0, 20.0) * (glow(length(d - far - vec2f(-4.0, 0.0)), 1.6) + glow(length(d - far - vec2f(4.0, 0.0)), 1.6)) * 0.9;
    col += rgb(255.0, 30.0, 20.0) * glow(length((d - far) * vec2f(0.4, 1.0)), 6.0) * 0.12;
    // foschia verso l'orizzonte
    col = mix(col, rgb(22.0, 18.0, 26.0), exp(-dy * 0.05) * 0.8);
    return col;
}

// ---------- materiali ----------
fn wood(d: vec2f) -> vec3f {
    // radica di noce: venature larghe e contorte, piccoli "occhi" scuri, poro fine
    let q = d * 0.0065;
    let warp = vec2f(fbm(q * 1.6 + 3.1), fbm(q * 1.6 + 7.7));
    let w1 = fbm(q + warp * 1.6);
    let rings = 0.5 + 0.5 * sin((w1 * 13.0 + fbm(q * 5.0) * 1.5) * PI);
    var c = mix(rgb(44.0, 17.0, 8.0), rgb(104.0, 44.0, 19.0), smoothstep(0.1, 0.9, rings));
    c = mix(c, rgb(150.0, 78.0, 34.0), smoothstep(0.86, 1.0, rings) * 0.45);
    let eyes = vnoise(d * 0.06 + warp * 4.0);
    c = mix(c, rgb(26.0, 9.0, 5.0), smoothstep(0.82, 0.95, eyes) * 0.7);
    c *= 0.92 + 0.12 * vnoise(d * vec2f(0.9, 0.25) + warp * 8.0);
    return c;
}

fn wood_panel(col: vec3f, d: vec2f, c: vec2f, h: vec2f, r: f32, px: f32) -> vec3f {
    let sd = sd_rbox(d, c, h, r);
    if (sd > 14.0) { return col; }
    // ombra sotto il pannello
    var out = col * (1.0 - 0.45 * (1.0 - smoothstep(0.0, 14.0, sd)) * step(0.0, sd));
    var w = wood(d) * 0.66;
    // smusso: luce dall'alto sul bordo superiore, scuro su quello inferiore
    let e = -sd;
    let bevel = 1.0 - smoothstep(0.0, 7.0, e);
    let ny = (d.y - c.y) / h.y;
    w *= 1.0 + bevel * (-ny) * 0.55;
    // lacca: riflesso morbido di una luce della cabina
    w += vec3f(1.0, 0.85, 0.7) * 0.07 * exp(-pow((d.y - (c.y - h.y) - 26.0) / 30.0, 2.0)) * (0.6 + 0.4 * sin(d.x * 0.004));
    w += vec3f(0.12, 0.08, 0.05) * pow(sat(1.0 - abs(e - 2.0) / 2.0), 2.0) * step(ny, 0.0);
    return mix(out, w, inside(sd, px));
}

fn recess(col: vec3f, d: vec2f, c: vec2f, h: vec2f, r: f32, px: f32, base: vec3f) -> vec3f {
    let sd = sd_rbox(d, c, h, r);
    let e = -sd;
    var b = base * (0.55 + 0.45 * smoothstep(0.0, 22.0, e));
    // bordo superiore in ombra, inferiore che prende luce
    let ny = (d.y - c.y) / h.y;
    b += vec3f(0.05) * (1.0 - smoothstep(0.0, 3.0, e)) * sat(ny);
    return mix(col, b, inside(sd, px));
}

fn dial_face(col: vec3f, d: vec2f, g: vec4f, px: f32) -> vec3f {
    let c = g.xy;
    let r = g.z;
    let dist = length(d - c);
    var out = col;
    let kind = g.w;
    // ombra attorno
    out *= 1.0 - 0.5 * (1.0 - smoothstep(r, r + 12.0, dist));
    let dir = (d - c) / max(dist, 1e-3);
    let shade = 0.5 + 0.5 * dot(dir, normalize(vec2f(-0.45, -0.9)));
    var ring_w = 5.0;
    var ring_c = rgb(78.0, 80.0, 86.0) * (0.45 + 0.9 * shade);
    if (kind > 0.5 && kind < 1.5) {
        ring_w = 2.5;
        ring_c = rgb(52.0, 54.0, 58.0) * (0.6 + 0.6 * shade);
    } else if (kind > 1.5) {
        // orologio: ghiera cromata
        ring_w = 9.0;
        let band = sin((dist - r) * 0.9) * 0.5 + 0.5;
        ring_c = mix(rgb(70.0, 72.0, 78.0), rgb(210.0, 212.0, 218.0), shade * (0.6 + 0.4 * band));
    }
    // quadrante nero con un filo di retroilluminazione calda
    var face = rgb(7.0, 8.0, 10.0);
    face += rgb(26.0, 18.0, 10.0) * smoothstep(r * 0.55, r, dist) * 0.6;
    face += vec3f(0.02) * (1.0 - dist / r);
    out = mix(out, ring_c, inside(dist - r - ring_w, px));
    out = mix(out, face, inside(dist - r, px));
    return out;
}

fn knob(col: vec3f, d: vec2f, c: vec2f, r: f32, px: f32) -> vec3f {
    let v = d - c;
    let dist = length(v);
    var out = col * (1.0 - 0.5 * (1.0 - smoothstep(r, r + 16.0, dist - 4.0)));
    // anello con le posizioni stampate
    let a = atan2(v.y, v.x);
    let ticks = step(0.85, fract(a / (2.0 * PI) * 8.0 + 0.06)) * step(abs(dist - r - 12.0), 3.0) * step(0.2, sin(a) + 0.9);
    out = mix(out, rgb(200.0, 200.0, 190.0) * 0.6, ticks);
    let knurl = 0.5 + 0.5 * sin(a * 48.0);
    let shade = 0.5 + 0.5 * dot(v / max(dist, 1e-3), normalize(vec2f(-0.4, -0.9)));
    var body = rgb(30.0, 31.0, 34.0) * (0.6 + 0.6 * shade);
    body = mix(body, body * (0.7 + 0.5 * knurl), smoothstep(r * 0.78, r * 0.86, dist));
    // indicatore
    let ind = seg(d, c, c + vec2f(cos(-2.2), sin(-2.2)) * r * 0.7);
    body = mix(body, rgb(210.0, 210.0, 200.0), inside(ind.x - 2.5, px));
    return mix(out, body, inside(dist - r, px));
}

fn scene(p: vec2f) -> vec3f {
    let k = dk();
    let o = dorigin();
    let d = (p - o) / k;
    let px = 0.8 / k;

    var col = outside(d, px);

    // cielo della cabina (rivestimento) e montanti, che coprono anche gli schermi molto larghi
    let roof = 46.0 + 70.0 * pow(abs(d.x - 960.0) / 1300.0, 2.0);
    col = mix(col, rgb(12.0, 12.0, 14.0) + vec3f(0.03) * sat(d.y / roof), inside(d.y - roof, px * 3.0));
    let hood = 468.0 + 46.0 * pow((d.x - 960.0) / 960.0, 2.0);
    let pil = 0.025 * u.canvas.x + (hood * k + o.y - p.y) * 0.22;
    col = mix(col, rgb(10.0, 10.0, 12.0), inside(p.x - pil, 1.5));
    col = mix(col, rgb(10.0, 10.0, 12.0), inside((u.canvas.x - p.x) - pil, 1.5));

    // ---- plancia in plastica ----
    if (d.y > hood - 3.0) {
        let grain = fbm(d * 0.35) * 0.05 + vnoise(d * 1.7) * 0.025;
        var dash = rgb(36.0, 39.0, 44.0) * (0.85 + grain * 3.0);
        dash *= mix(1.0, 0.45, sat((d.y - hood) / 620.0));
        // labbro superiore che prende la luce della strada
        dash += rgb(90.0, 96.0, 108.0) * exp(-(d.y - hood) / 5.0) * 0.6;
        // luce calda degli strumenti sulla plastica
        dash += rgb(70.0, 34.0, 10.0) * glow(length((d - vec2f(880.0, 735.0)) * vec2f(0.55, 1.0)), 420.0) * 0.12;
        col = mix(col, dash, inside(hood - d.y, px));
    }

    // visiera sopra il quadro: ombra sulla radica
    let vis = sd_rbox(d, vec2f(876.0, 528.0), vec2f(540.0, 22.0), 20.0);
    col = mix(col, rgb(24.0, 26.0, 30.0), inside(vis, px));

    // ---- pannelli in radica ----
    col = wood_panel(col, d, vec2f(876.0, 735.0), vec2f(516.0, 197.0), 26.0, px);
    col = wood_panel(col, d, vec2f(1635.0, 735.0), vec2f(205.0, 197.0), 22.0, px);
    col = wood_panel(col, d, vec2f(210.0, 735.0), vec2f(120.0, 197.0), 22.0, px);
    // ombra della visiera sulla radica del quadro
    col *= 1.0 - 0.45 * exp(-max(d.y - 548.0, 0.0) / 26.0) * step(abs(d.x - 876.0), 520.0) * step(538.0, d.y) * step(d.y, 640.0);

    // quadro strumenti: vano nero
    col = recess(col, d, vec2f(876.0, 735.0), vec2f(488.0, 166.0), 16.0, px, rgb(13.0, 14.0, 16.0));
    // separatori sottili tra i gruppi
    for (var i = 0; i < 2; i++) {
        let x = select(658.0, 1236.0, i == 1);
        col = mix(col, rgb(24.0, 25.0, 28.0), inside(abs(d.x - x) - 1.0, px) * step(abs(d.y - 735.0), 150.0));
    }

    // quadranti
    var gs = GAUGES;
    for (var i = 0u; i < N_GAUGES; i++) {
        col = dial_face(col, d, gs[i], px);
    }

    // display LCD e alloggi degli interruttori
    col = recess(col, d, LCD_BOX.xy, LCD_BOX.zw, 6.0, px, rgb(16.0, 17.0, 18.0));
    var rs = ROCKERS;
    for (var i = 0u; i < 12u; i++) {
        col = recess(col, d, rs[i].xy, rs[i].zw + vec2f(5.0), 7.0, px, rgb(14.0, 14.0, 16.0));
    }
    for (var i = 0; i < 3; i++) {
        col = recess(col, d, vec2f(150.0 + 60.0 * f32(i), 872.0), vec2f(28.0, 37.0), 7.0, px, rgb(14.0, 14.0, 16.0));
    }

    // console di sinistra: bocchetta dell'aria e manopola delle luci
    let vent = sd_rbox(d, vec2f(210.0, 604.0), vec2f(92.0, 36.0), 8.0);
    var vc = rgb(10.0, 10.0, 12.0);
    vc += rgb(40.0, 42.0, 46.0) * (1.0 - smoothstep(1.0, 3.0, abs(fract((d.y - 568.0) / 10.0) - 0.5) * 10.0)) * 0.6;
    col = mix(col, rgb(34.0, 35.0, 38.0), inside(vent - 5.0, px));
    col = mix(col, vc, inside(vent, px));
    col = knob(col, d, vec2f(210.0, 742.0), 46.0, px);

    // ---- volante ----
    let wc = vec2f(876.0, 1540.0);
    let wr = 588.0;
    let wd = length(d - wc) - wr;
    let tk = 25.0;
    col *= 1.0 - 0.35 * exp(-max(abs(wd) - tk, 0.0) / 30.0) * step(wd, 0.0);
    if (abs(wd) < tk + 2.0) {
        let n = wd / tk;
        let lit = sqrt(sat(1.0 - n * n));
        var rim = rgb(18.0, 18.0, 20.0) * (0.5 + 0.8 * lit);
        rim += rgb(70.0, 72.0, 80.0) * pow(sat(1.0 - abs(n + 0.35) * 2.2), 3.0) * 0.6;
        rim *= 0.9 + 0.2 * vnoise(d * 0.9);
        col = mix(col, rim, inside(abs(wd) - tk, px));
    }

    // vignettatura notturna
    let vq = (p / u.canvas) - vec2f(0.5, 0.55);
    col *= 1.0 - 0.35 * dot(vq, vq);
    return col;
}
