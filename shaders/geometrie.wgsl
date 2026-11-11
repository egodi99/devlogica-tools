// nome: Geometrie
// Rombi concentrici con gli angoli del marchio, chevron < >, triangoli, esagoni, orbita e pannelli di vetro.

fn logo_layout() -> vec3f {
    let w = min(0.26 * u.canvas.x, 0.46 * u.canvas.y) / 0.9;
    return vec3f(0.09 * u.canvas.x + w * 0.5, 0.5 * u.canvas.y, w);
}

// Le forme sono descritte in modo analitico (niente array indicizzati: sulla GPU finiscono in memoria lenta)
// e ogni pixel le valuta solo se cade vicino al loro raggio d'ingombro.

// Margine oltre il contorno entro cui contorno e impulsi lasciano luce (stroke più largo: 10 px di riferimento).
fn reach() -> f32 { return 6.0 * kpx() + 1.0; }

// Riempimento "vetro": sfumatura diagonale molto trasparente, sommata alla luce sotto.
// d = pixel - centro del riquadro della forma, r = metà del lato maggiore del riquadro.
fn glass_shade(d: vec2f, r: f32, col: vec3f, a: f32) -> vec3f {
    let s = sat((dot(d, vec2f(0.62161, -0.78333)) / r + 1.0) * 0.5);
    let dark = rgb(6.0, 5.0, 20.0);
    return select(mix(mix(col, dark, 0.6) * a * 0.5, col * a * 0.08, (s - 0.55) / 0.45), mix(col * a, mix(col, dark, 0.6) * a * 0.5, s / 0.55), s < 0.55);
}

fn outline(d: f32, w: f32) -> f32 { return stroke(d, 10.0) * 0.07 + stroke(d, 4.0) * 0.18 + stroke(d, w); }

// Impulso luminoso che corre lungo un contorno. q = posizione del pixel sul perimetro.
fn pulse(d: f32, q: f32, pos: f32, len: f32, closed: bool, c: vec3f) -> vec3f {
    let back = select(pos - q, fract(pos - q), closed);
    if (back < 0.0 || back > len) { return vec3f(0.0); }
    let hot = mix(c, vec3f(1.0), 0.65);
    var o = vec3f(0.0);
    o += c * 0.12 * stroke(d, 8.0);
    if (back < len * 0.6) { o += c * 0.45 * stroke(d, 3.0); }
    if (back < len * 0.25) { o += hot * stroke(d, 1.6); }
    return o;
}

// Rombo (semiassi 0.84·s orizzontale, s verticale) nel suo sistema locale q:
// distanza dal contorno e posizione sul perimetro (stesso verso dei vertici alto, destra, basso, sinistra).
fn rhomb_d(q: vec2f, s: f32) -> vec2f {
    let a = abs(q);
    let sg = seg(a, vec2f(0.0, s), vec2f(0.84 * s, 0.0));
    let i = select(select(3.0, 2.0, q.y >= 0.0), select(0.0, 1.0, q.y >= 0.0), q.x >= 0.0);
    let h = select(sg.y, 1.0 - sg.y, i == 1.0 || i == 3.0);
    return vec2f(sg.x, (i + h) * 0.25);
}

// Triangolo equilatero di raggio s (vertici in alto, basso a destra, basso a sinistra) nel sistema locale q.
fn tri_d(q: vec2f, s: f32) -> vec2f {
    let a = vec2f(0.0, -s);
    let b = vec2f(0.866 * s, 0.5 * s);
    let c = vec2f(-0.866 * s, 0.5 * s);
    let s0 = seg(q, a, b);
    let s1 = seg(q, b, c);
    let s2 = seg(q, c, a);
    var r = vec2f(s0.x, s0.y / 3.0);
    if (s1.x < r.x) { r = vec2f(s1.x, (1.0 + s1.y) / 3.0); }
    if (s2.x < r.x) { r = vec2f(s2.x, (2.0 + s2.y) / 3.0); }
    return r;
}

fn tri_in(q: vec2f, s: f32) -> bool {
    // tre semipiani dei lati (apotema s/2)
    let n0 = dot(q, vec2f(0.866, -0.5));
    let n1 = q.y;
    let n2 = dot(q, vec2f(-0.866, -0.5));
    return max(max(n0, n1), n2) < 0.5 * s;
}

// Esagono regolare di raggio s con vertici agli angoli k·60° nel sistema locale q.
// .x distanza dal contorno, .y posizione sul perimetro, .z angolo polare (0..2π).
fn hex_d(q: vec2f, s: f32) -> vec3f {
    var an = atan2(q.y, q.x);
    if (an < 0.0) { an += 2.0 * PI; }
    let i = min(floor(an / (PI / 3.0)), 5.0);
    let qs = rot2(-(i + 0.5) * PI / 3.0) * q;
    let h = clamp((qs.y + 0.5 * s) / s, 0.0, 1.0);
    let d = length(vec2f(qs.x - 0.866025 * s, qs.y - (h - 0.5) * s));
    return vec3f(d, (i + h) / 6.0, an);
}

// Solo distanza dal contorno dell'esagono (stessa orientazione), senza trigonometria.
fn hex_dist(q: vec2f, s: f32) -> f32 {
    let kk = vec3f(-0.866025404, 0.5, 0.577350269);
    var a = abs(q);
    a -= 2.0 * min(dot(kk.xy, a), 0.0) * kk.xy;
    let ap = s * 0.866025404;
    a -= vec2f(clamp(a.x, -kk.z * ap, kk.z * ap), ap);
    return length(a);
}

// Valori di hash11(j + 20/30/40/50/60) dei pannelli, precalcolati (come li calcola la GPU) per non rifarli a ogni pixel.
fn panel_h(j: i32) -> vec4f {
    switch j {
        case 0: { return vec4f(0.8203125, 0.9335938, 0.2539062, 0.6054688); }
        case 1: { return vec4f(0.3105469, 0.53125, 0.0776367, 0.3671875); }
        case 2: { return vec4f(0.1601562, 0.25, 0.21875, 0.2890625); }
        case 3: { return vec4f(0.8496094, 0.359375, 0.5546875, 0.3164062); }
        case 4: { return vec4f(0.7636719, 0.6171875, 0.3359375, 0.046875); }
        case 5: { return vec4f(0.484375, 0.4316406, 0.5859375, 0.8320312); }
        default: { return vec4f(0.5595703, 0.5, 0.5546875, 0.28125); }
    }
}
fn panel_al(j: i32) -> f32 {
    switch j {
        case 0: { return 0.2929688; }
        case 1: { return 0.8017578; }
        case 2: { return 0.3203125; }
        case 3: { return 0.8237305; }
        case 4: { return 0.4179688; }
        case 5: { return 0.3369141; }
        default: { return 0.765625; }
    }
}

// cos e sin di un angolo piccolo (|x| < 0.3) con un polinomio: niente trigonometria vera per pixel.
fn sc_small(x: f32) -> vec2f {
    let x2 = x * x;
    return vec2f(1.0 - x2 * (0.5 - x2 * (1.0 / 24.0 - x2 / 720.0)), x * (1.0 - x2 * (1.0 / 6.0 - x2 / 120.0)));
}

// Lampo periodico: intensità per la forma id (0..3 rombi, 4 chevron, 5 triangoli). Calcolato solo vicino ai contorni.
fn flash(id: i32) -> f32 {
    let t = u.time;
    if (t > 4.0 && u.since > 3.0) {
        let fc = floor((t - 4.0) / 7.5);
        if (i32(floor(hash11(fc + 9.0) * 6.0)) == id) { return sin(min(1.0, ((t - 4.0) - fc * 7.5) / 2.0) * PI); }
    }
    return 0.0;
}

fn rhomb_col(j: i32, AZ: vec3f, IND: vec3f, VI: vec3f, LI: vec3f) -> vec3f {
    return select(select(LI, VI, j == 2), select(IND, AZ, j == 0), j < 2);
}

// hash intero (pcg) per la polvere: stabile anche con tempi grandi
fn pcg3(v0: vec3u) -> vec3u {
    var v = v0 * 1664525u + 1013904223u;
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    v ^= v >> vec3u(16u);
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    return v;
}
fn h01(x: u32) -> f32 { return f32(x >> 8u) / 16777216.0; }

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let A = ease_out((u.since - 0.2) / 2.6);
    let CX = 0.73 * W;
    let CY = 0.5 * H;
    let RE = reach();

    let SKY = rgb(112.0,189.0,232.0);
    let AZ = rgb(49.0,140.0,211.0);
    let IND = rgb(90.0,100.0,240.0);
    let VI = rgb(134.0,18.0,173.0);
    let LI = rgb(194.0,136.0,214.0);
    let MAG = rgb(200.0,110.0,240.0);

    // sfondo e luci
    var col = rgb(4.0, 3.0, 12.0);
    // seni condivisi con i piani del punto 1 (sin(a + 1) = sin a·cos 1 + cos a·sin 1)
    let s07 = vec2f(sin(t * 0.07), cos(t * 0.07));
    let s05 = vec2f(sin(t * 0.05), cos(t * 0.05));
    let g1 = vec2f(W * (0.74 + 0.05 * s07.x), H * (0.3 + 0.08 * cos(t * 0.09)));
    let d1 = length(p - g1) / (max(W, H) * 0.7);
    col = mix(col, select(rgb(26.0,20.0,110.0), mix(rgb(40.0,48.0,190.0), rgb(26.0,20.0,110.0), d1 / 0.5), d1 < 0.5),
              select(mix(0.14, 0.0, sat((d1 - 0.5) / 0.5)), mix(0.38 + 0.08 * sin(t * 0.5), 0.14, d1 / 0.5), d1 < 0.5));
    let d2 = length(p - vec2f(0.9 * W, 0.95 * H)) / (max(W, H) * 0.55);
    col = mix(col, rgb(120.0,24.0,170.0), (0.26 + 0.07 * sin(t * 0.43 + 1.0)) * sat(1.0 - d2));

    // 1) grandi piani scuri in diagonale
    for (var j = 0; j < 2; j++) {
        let fj = f32(j);
        let c = select(AZ, VI, j == 0);
        let sx = select(s05.x * 0.5403023 + s05.y * 0.8414710, s05.x, j == 0);
        let ctr = select(vec2f(0.66 * W, 1.0 * H), vec2f(0.9 * W, 0.62 * H), j == 0) + vec2f(sx * H * 0.02, 0.0);
        let e = 0.015 * select(s07.x * 0.5403023 + s07.y * 0.8414710, s07.x, j == 0);
        // rotazione di -r con r = -0.87 + e: cos/sin di 0.87 costanti, e piccolo
        let ce = sc_small(e);
        let cr = 0.6448265 * ce.x + 0.7643289 * ce.y;
        let sr = -0.7643289 * ce.x + 0.6448265 * ce.y;
        let mr = mat2x2f(cr, -sr, sr, cr);
        let l = select(3.0, 3.2, j == 0) * H;
        let w = select(0.16, 0.3, j == 0) * H;
        let lp = mr * (p - ctr);
        if (abs(lp.x) < l * 0.5 && abs(lp.y) < w * 0.5) {
            let s = (lp.y + w * 0.5) / w;
            col = mix(col, mix(mix(rgb(8.0,6.0,24.0), c, 0.2), rgb(5.0,4.0,14.0), s), 0.85 * A);
        }
        let d = abs(lp.y + w * 0.5);
        if (abs(lp.x) < l * 0.5 && d < RE) {
            let ec = mix(c, vec3f(1.0), 0.2);
            col += ec * outline(d, 1.3) * 0.35 * A;
            col += pulse(d, (lp.x + l * 0.5) / l, ((t * 0.08 + fj * 0.5) % 1.4) - 0.1, 0.18, false, mix(c, vec3f(1.0), 0.3)) * A;
        }
    }

    // 2) pannelli di vetro che scorrono a ~50°, come i bracci del logo
    let pr = -0.87;
    let mpr = rot2(-pr);
    // i pannelli sono paralleli e distano 0.22·H l'uno dall'altro (più larghi di 0.04·H non sono):
    // ogni pixel può toccarne al massimo uno, quello della sua fascia.
    let lp0 = mpr * (p - vec2f(0.78 * W, 0.5 * H));
    {
        let j = i32(clamp(round((lp0.y / H + 0.66) / 0.22), 0.0, 6.0));
        let fj = f32(j);
        let hh = panel_h(j);
        let sp = 0.012 + 0.02 * hh.x;
        let uu = (hh.y * 1.4 + sp * t) % 1.4;
        let vv = (fj / 6.0) * 1.2 - 0.1;
        let l = (0.35 + 0.5 * hh.z) * H;
        let w = (0.025 + 0.05 * hh.w) * H;
        let along = (uu - 0.2) * length(u.canvas) * 0.9;
        let across = (vv - 0.5) * H * 1.1;
        let lp = lp0 - vec2f(along, across);
        let d = abs(lp.y + w * 0.5);
        if (abs(lp.x) < l * 0.5 && (abs(lp.y) < w * 0.5 || d < RE)) {
            var pc = MAG;
            switch j {
                case 0: { pc = AZ; }
                case 1: { pc = IND; }
                case 2: { pc = VI; }
                case 3: { pc = LI; }
                case 4: { pc = SKY; }
                case 5: { pc = rgb(85.0,0.0,124.0); }
                default: {}
            }
            let al = 0.06 + 0.09 * panel_al(j);
            let fa = A * min(1.0, sin(PI * sat(uu / 1.4)) * 2.0);
            if (abs(lp.y) < w * 0.5) {
                let s = (lp.y + w * 0.5) / w;
                let dark = rgb(6.0, 5.0, 20.0);
                col += select(mix(mix(pc, dark, 0.6) * 0.5, pc * 0.08, (s - 0.55) / 0.45), mix(pc, mix(pc, dark, 0.6) * 0.5, s / 0.55), s < 0.55) * al * fa;
            }
            col += mix(pc, vec3f(1.0), 0.25) * outline(d, 1.0) * 0.45 * fa;
        }
    }

    // 3) orbita tratteggiata con satellite
    let orR = 0.41 * H;
    let ep = rot2(0.35) * (p - vec2f(CX, CY));
    let er = length(ep / vec2f(orR, orR * 0.92));
    let ed = abs(er - 1.0) * orR * 0.96;
    if (ed < RE) {
        let arc = (atan2(ep.y / 0.92, ep.x) + PI) * orR;
        let dash = step(fract((arc + t * 20.0 * k) / (12.0 * k)), 2.0 / 12.0);
        col += LI * 0.28 * A * dash * stroke(ed, 1.2);
    }
    if (ed < 10.0 * k) {
        let sa = t * 0.35;
        let sat_p = vec2f(CX, CY) + rot2(-0.35) * vec2f(cos(sa) * orR, sin(sa) * orR * 0.92);
        let sdd = length(p - sat_p) / (9.0 * k);
        if (sdd < 1.0) { col += select(SKY * 0.5 * (1.0 - (sdd - 0.4) / 0.6), mix(vec3f(0.9), SKY * 0.5, sdd / 0.4), sdd < 0.4) * A; }
    }

    // 4) rombi concentrici
    let pc4 = p - vec2f(CX, CY);
    let pl4 = length(pc4);
    if (pl4 < 0.30 * 1.035 * H + RE) {
        let br = 1.0 + 0.035 * sin(t * 0.6);
        let rsc = H * br * mix(0.6, 1.0, A);
        for (var j = 0; j < 4; j++) {
            let fj = f32(j);
            var rs = 0.105;
            var rr = -0.2 * sin(t * 0.18);
            switch j {
                case 0: { rs = 0.30; rr = 0.06 * sin(t * 0.12); }
                case 1: { rs = 0.235; rr = -0.09 * sin(t * 0.15 + 1.0); }
                case 2: { rs = 0.17; rr = 0.14 * sin(t * 0.1 + 2.0); }
                default: {}
            }
            let s = rsc * rs;
            if (pl4 < s + RE) {
                let csn = sc_small(rr);
                let cs = csn.x;
                let sn = csn.y;
                // sistema locale: rotazione inversa di quella dei vertici
                let q = vec2f(cs * pc4.x + sn * pc4.y, -sn * pc4.x + cs * pc4.y);
                let c = rhomb_col(j, AZ, IND, VI, LI);
                if ((j == 1 || j == 3) && abs(q.x) / 0.84 + abs(q.y) < s) {
                    let r = max(max(abs(sn), 0.84 * abs(cs)), max(abs(cs), 0.84 * abs(sn))) * s;
                    col += glass_shade(pc4, r, c, 0.16 * A);
                }
                let pd = rhomb_d(q, s);
                if (pd.x < RE) {
                    let f = flash(j);
                    col += mix(c, vec3f(1.0), 0.15 + f * 0.4) * outline(pd.x, 1.3) * (0.42 + f * 0.5) * A;
                    let sgn = select(1.0, -1.0, j % 2 == 1);
                    col += pulse(pd.x, pd.y, t * (0.06 + fj * 0.025) * sgn + fj * 0.3, 0.16, true, mix(c, LI, 0.3)) * A;
                }
            }
        }
    }

    // 5) chevron < > ai lati dei rombi
    let cs5 = H * 0.075;
    // a sinistra del centro solo «<», a destra solo «>»
    {
        let j = select(1, 0, p.x < CX);
        let sgn = select(1.0, -1.0, j == 0);
        // prova d'ingombro sul centro medio (lo scarto oscilla di ±0.025·H)
        let q0 = p - vec2f(CX + sgn * 0.33 * H, CY);
        let rb = 1.1 * cs5 + 0.025 * H + RE;
        if (dot(q0, q0) < rb * rb) {
        let gap = H * (0.33 + 0.025 * sin(t * 0.8));
        let q = p - vec2f(CX + sgn * gap, CY);
        if (dot(q, q) < (1.1 * cs5 + RE) * (1.1 * cs5 + RE)) {
            let c = select(SKY, VI, j == 0);
            let a = vec2f(-0.45 * sgn, -1.0) * cs5;
            let b = vec2f(0.45 * sgn, 0.0) * cs5;
            let e = vec2f(-0.45 * sgn, 1.0) * cs5;
            let s0 = seg(q, a, b);
            let s1 = seg(q, b, e);
            let pd = select(vec2f(s0.x, s0.y * 0.5), vec2f(s1.x, 0.5 + s1.y * 0.5), s1.x < s0.x);
            if (pd.x < RE) {
                let f = flash(4);
                col += c * outline(pd.x, 2.0) * (0.55 + f * 0.4) * A;
                col += pulse(pd.x, pd.y, ((t * 0.25 + f32(j) * 0.5) % 1.6) - 0.3, 0.35, false, mix(c, vec3f(1.0), 0.3)) * A;
            }
        }
        }
    }

    // 6) triangoli che ruotano lenti
    // sono lontani tra loro: ogni quadrante della zona destra ne ha uno solo
    {
        let j = select(select(1, 3, p.y < 0.5 * H), select(2, 0, p.y < 0.5 * H), p.x < 0.78 * W);
        let fj = f32(j);
        var q4 = vec4f(0.97, 0.12, 0.04, -0.25);
        var tc = IND;
        switch j {
            case 0: { q4 = vec4f(0.585, 0.2, 0.065, 0.15); tc = SKY; }
            case 1: { q4 = vec4f(0.94, 0.78, 0.1, -0.1); tc = MAG; }
            case 2: { q4 = vec4f(0.62, 0.84, 0.045, 0.22); tc = LI; }
            default: {}
        }
        let s = H * q4.z;
        // prova d'ingombro sul centro fermo (l'oscillazione resta entro 0.0213·H)
        let dp0 = p - vec2f(W * q4.x, H * q4.y);
        let rb = s + 0.0213 * H + RE;
        if (dot(dp0, dp0) < rb * rb) {
        let ctr = vec2f(W * q4.x, H * q4.y) + vec2f(sin(t * 0.13 + fj), cos(t * 0.11 + fj)) * H * 0.015;
        let dp = p - ctr;
        if (dot(dp, dp) < (s + RE) * (s + RE)) {
            let ang = q4.w * t * 0.4 + fj;
            let cs = cos(ang);
            let sn = sin(ang);
            let q = vec2f(cs * dp.x + sn * dp.y, -sn * dp.x + cs * dp.y);
            if (tri_in(q, s)) {
                // riquadro dei vertici ruotati
                let m = mat2x2f(cs, sn, -sn, cs);
                let va = m * vec2f(0.0, -s);
                let vb = m * vec2f(0.866 * s, 0.5 * s);
                let vc = m * vec2f(-0.866 * s, 0.5 * s);
                let lo = min(min(va, vb), vc);
                let hi = max(max(va, vb), vc);
                let r = max(hi.x - lo.x, hi.y - lo.y) * 0.5;
                col += glass_shade(dp - (lo + hi) * 0.5, r, tc, 0.12 * A);
            }
            let pd = tri_d(q, s);
            if (pd.x < RE) {
                let f = flash(5);
                col += tc * outline(pd.x, 1.3) * (0.5 + f * 0.4) * A;
                col += pulse(pd.x, pd.y, t * 0.12 + fj * 0.25, 0.2, true, tc) * A;
            }
        }
        }
    }

    // 7) esagono doppio con nodi
    let hc = vec2f(0.915 * W, 0.24 * H);
    let hR = H * 0.085;
    let dh = p - hc;
    if (dot(dh, dh) < (hR + 7.0 * k + RE) * (hR + 7.0 * k + RE)) {
        let ha = t * 0.05;
        let ch = cos(ha);
        let sh = sin(ha);
        let q = vec2f(ch * dh.x + sh * dh.y, -sh * dh.x + ch * dh.y);
        let hd = hex_d(q, hR);
        col += IND * outline(hd.x, 1.3) * 0.4 * A;
        col += pulse(hd.x, hd.y, -t * 0.1, 0.18, true, SKY) * A;
        let hq = rot2(-t * 0.08);
        let q2 = vec2f(hq[0].x * dh.x + hq[0].y * dh.y, hq[1].x * dh.x + hq[1].y * dh.y);
        col += MAG * outline(hex_dist(q2, H * 0.05), 1.0) * 0.3 * A;
        // nodo sul vertice più vicino
        let m = round(hd.z / (PI / 3.0));
        let mi = m % 6.0;
        let vm = vec2f(cos(m * PI / 3.0), sin(m * PI / 3.0)) * hR;
        let dn = length(q - vm) / (6.0 * k);
        if (dn < 1.0) {
            let a = (0.4 + 0.6 * max(0.0, sin(t * 1.2 - mi * 1.05))) * A;
            col += select(SKY * 0.5 * a * (1.0 - (dn - 0.4) / 0.6), mix(vec3f(a), SKY * 0.5 * a, dn / 0.4), dn < 0.4);
        }
    }

    // 8) griglia di punti a rombo
    let gp = (p - vec2f(0.62 * W, 0.62 * H)) / H;
    if (gp.x > -0.03 && gp.x < 0.3 && gp.y > -0.03 && gp.y < 0.15) {
    let gj = round(gp.y / 0.042);
    let gi = round((gp.x - select(0.0, 0.025, i32(gj) % 2 == 1)) / 0.05);
    if (gi >= 0.0 && gi < 6.0 && gj >= 0.0 && gj < 4.0) {
        let cdot = vec2f(0.62 * W + gi * 0.05 * H + select(0.0, 0.025 * H, i32(gj) % 2 == 1), 0.62 * H + gj * 0.042 * H);
        let dd = abs(p - cdot);
        if (dd.x + dd.y < 1.6 * k) { col += LI * (0.12 + 0.25 * max(0.0, sin(t * 0.9 - gi * 0.5 - gj * 0.7))) * A; }
    }
    }

    // 9) polvere luminosa che sale: una particella per cella, in colonne che salgono ognuna alla sua velocità.
    //    La particella nasce dentro la sua colonna con lo spazio per la propria deriva laterale,
    //    così ogni pixel guarda soltanto la sua cella.
    if (A > 0.5 && p.x > 0.45 * W && p.x < 1.05 * W) {
        let cw = 0.06 * W;
        let chh = 0.18 * H;
        let cx = floor((p.x - 0.45 * W) / cw);
        let hcol = pcg3(vec3u(u32(cx), 77u, 913u));
        let rise = (0.006 + 0.014 * h01(hcol.x)) * H;
        // sistema della colonna: sale con le sue particelle (avvolto per non perdere precisione)
        let off = (t * rise) % (chh * 4096.0);
        let cy = floor((p.y + off) / chh);
        let hc0 = pcg3(vec3u(u32(cx), u32(i32(cy) + 100000), 5u));
        let pd = 6.0 + 8.0 * h01(hc0.x);
        let tt0 = t + h01(hc0.y) * pd;
        let cyc = floor(tt0 / pd);
        let lt = tt0 - cyc * pd;
        let hp = pcg3(vec3u(hc0.z, u32(cyc), 31u));
        let rad = (0.6 + 1.5 * h01(hp.z)) * 2.6 * k;
        let vx = (h01(hc0.x ^ hp.x) - 0.3) * 0.004 * W;
        let span = vx * pd;
        let xa = cx * cw + max(0.0, -span) + rad;
        let xb = (cx + 1.0) * cw - max(0.0, span) - rad;
        let x0 = 0.45 * W + mix(xa, xb, h01(hp.x));
        let fy0 = (cy + mix(0.05, 0.95, h01(hp.y))) * chh;
        // posizione alla nascita: deve cadere nella fascia di partenza originale
        let y_born = fy0 - off + rise * lt;
        let pos = vec2f(x0 + vx * lt, fy0 - off);
        let d = length(p - pos);
        if (d < rad && y_born > 0.2 * H && y_born < 1.05 * H) {
            let f = min(1.0, lt / 1.5) * sat((pd - lt) / 1.5) * (0.55 + 0.45 * sin(t * 2.0 + h01(hp.x ^ hp.y) * 50.0));
            let ci = (hp.x ^ hp.z) & 3u;
            let c = select(select(IND, MAG, ci == 2u), select(LI, SKY, ci == 0u), ci < 2u);
            let x = d / rad;
            col += select(c * 0.3 * (1.0 - (x - 0.4) / 0.6), mix(vec3f(0.65), c * 0.3, x / 0.4), x < 0.4) * f;
        }
    }

    // lato sinistro più scuro per il logo, vignettatura
    let xr = p.x / (0.55 * W);
    let va = select(select(0.0, mix(0.3, 0.0, (xr - 0.6) / 0.4), xr < 1.0), mix(0.85, 0.3, xr / 0.6), xr < 0.6);
    col = mix(col, rgb(3.0, 2.0, 10.0), va);
    let dv = length(p - u.canvas * 0.5);
    let r0 = min(W, H) * 0.4;
    let r1 = length(u.canvas) * 0.62;
    col *= 1.0 - 0.5 * sat((dv - r0) / (r1 - r0));
    return col;
}
