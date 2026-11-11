// nome: Lame di luce
// Ventaglio di lame stratificate con bordi al neon, riflessi, lampi e polvere luminosa.

fn logo_layout() -> vec3f {
    let w = min(0.26 * u.canvas.x, 0.46 * u.canvas.y) / 0.9;
    return vec3f(0.09 * u.canvas.x + w * 0.5, 0.5 * u.canvas.y, w);
}

// gradiente a 4 tappe (valori scalari o colori separati)
fn ramp4(x: f32, a: f32, b: f32, c: f32, d: f32, sb: f32, sc: f32) -> f32 {
    if (x < sb) { return mix(a, b, sat(x / sb)); }
    if (x < sc) { return mix(b, c, (x - sb) / (sc - sb)); }
    return mix(c, d, sat((x - sc) / (1.0 - sc)));
}

// costanti di ogni lama: con uno switch invece di array locali indicizzati
// (gli array locali letti con indice variabile finiscono in memoria lenta)
struct Lama { a0: f32, a1: f32, ph: f32, sheen: f32, edge: vec3f, body: vec3f };

fn lama(i: i32) -> Lama {
    switch (i) {
        case 0: { return Lama(-31.0, -38.5, 0.0, 0.55, rgb(112.0,189.0,232.0), rgb(49.0,140.0,211.0)); }
        case 1: { return Lama(-25.5, -31.6, 1.3, 0.75, rgb(150.0,170.0,255.0), rgb(52.0,60.0,200.0)); }
        case 2: { return Lama(-20.0, -26.0, 2.1, 0.9, rgb(194.0,136.0,214.0), rgb(96.0,58.0,214.0)); }
        case 3: { return Lama(-14.5, -20.4, 3.4, 0.7, rgb(214.0,150.0,240.0), rgb(134.0,18.0,173.0)); }
        case 4: { return Lama(-9.5, -14.9, 4.2, 0.5, rgb(194.0,136.0,214.0), rgb(104.0,24.0,160.0)); }
        default: { return Lama(-5.0, -9.9, 5.0, 0.4, rgb(170.0,120.0,230.0), rgb(85.0,0.0,124.0)); }
    }
}

// filamenti: angolo, intensità, velocità, colore
struct Filo { a: f32, al: f32, sp: f32, col: vec3f };

fn filo(j: i32) -> Filo {
    switch (j) {
        case 0: { return Filo(-36.5, 0.3, 0.13, rgb(112.0,189.0,232.0)); }
        case 1: { return Filo(-28.4, 0.2, 0.09, rgb(150.0,170.0,255.0)); }
        case 2: { return Filo(-17.3, 0.22, 0.11, rgb(194.0,136.0,214.0)); }
        case 3: { return Filo(-11.6, 0.16, 0.07, rgb(214.0,150.0,240.0)); }
        default: { return Filo(-6.2, 0.16, 0.1, rgb(150.0,120.0,230.0)); }
    }
}

// hash intero (pcg): niente seni, molti bit casuali da una sola chiamata
fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}
fn bits10(h: u32, sh: u32) -> f32 { return f32((h >> sh) & 1023u) / 1023.0; }

// polvere: uno strato di celle radiali che scorrono verso l'esterno lungo la lama b.
// Ogni pixel guarda solo la particella della propria cella.
fn polvere(b: i32, layer: u32, vel: f32, hc: f32, rr: f32, r: f32, ang: f32, lo: f32, hi: f32, L: f32, k: f32, t: f32, ec: vec3f) -> vec3f {
    let s = (rr - vel * t) / hc;
    let c = floor(s);
    let h = pcg(bitcast<u32>(i32(c)) * 3u + layer * 0x9E3779B9u + u32(b) * 0x85EBCA6Bu);
    let h2 = pcg(h);
    // posizione nella cella con margine, così la particella non sconfina
    let rq = (c + 0.2 + 0.6 * bits10(h, 0u)) * hc + vel * t;
    let rb = 0.12 + 0.1 * bits10(h, 10u);
    let pd = 4.0 + 7.0 * bits10(h, 20u);
    let lt = (rq - rb) / vel;
    if (lt <= 0.0 || lt >= pd) { return vec3f(0.0); }
    let aq = mix(lo, hi, bits10(h2, 0u));
    let rad = (0.6 + 1.6 * bits10(h2, 10u)) * 2.6 * k;
    let dr = r - rq * L;
    let da = r * radians(ang - aq);
    let d2 = dr * dr + da * da;
    if (d2 >= rad * rad) { return vec3f(0.0); }
    let x = sqrt(d2) / rad;
    let fd = min(1.0, lt / 1.5) * sat((pd - lt) / 1.5) * (0.6 + 0.4 * sin(t * 2.0 + 6.28 * bits10(h2, 20u)));
    let c0 = select(mix(ec * 0.35, vec3f(0.0), (x - 0.4) / 0.6), mix(vec3f(0.7), ec * 0.35, x / 0.4), x < 0.4);
    return c0 * fd;
}

// dati del pixel che servono a ogni lama
struct Geo { r: f32, rr: f32, ang: f32, L: f32, k: f32, t: f32, open: f32, flareK: f32, fb: i32, mdust: f32 };
// lama più in alto trovata finora e luce additiva accumulata
struct Stato { top: i32, lo: f32, hi: f32, f: f32, acc: vec3f };

// modulo per valori positivi: più economico del % in virgola mobile
fn modp(x: f32, m: f32) -> f32 { return x - m * floor(x / m); }

// impulso lungo il bordo: .x = nucleo caldo, .y = alone
fn impulso(tt: f32, pos: f32) -> vec2f {
    if (pos > 0.2 && pos < 0.98) {
        let q = abs(tt - pos);
        return vec2f(sat(1.0 - q / 0.1), 0.18 * exp(-q * q / 0.02));
    }
    return vec2f(0.0);
}

// profilo triangolare di un riflesso centrato in sw
fn riflesso(tt: f32, sw: f32) -> f32 {
    if (tt > sw - 0.22 && tt <= sw) { return (tt - sw + 0.22) / 0.22; }
    if (tt > sw && tt < sw + 0.26) { return 1.0 - (tt - sw) / 0.26; }
    return 0.0;
}

// una lama: posizione, bordo al neon e polvere. Chiamata sei volte con indice costante
// (srotolata a mano: con un ciclo il compilatore genera codice molto più lento)
fn lamina(i: i32, st: ptr<function, Stato>, g: Geo) {
    let B = lama(i);
    let fi = f32(i);
    let t = g.t;
    let r = g.r;
    let ang = g.ang;
    let k = g.k;
    let L = g.L;
    let open = g.open;
    let breath = 2.2 * sin(t * 0.22) * (fi / 5.0 - 0.5);
    let ripple = 1.6 * sin(t * 0.55 - fi * 0.75);
    let wob = 2.2 * sin(t * 0.21 + B.ph) + 0.9 * sin(t * 0.083 + B.ph * 2.0) + breath;
    let lo = B.a0 + wob;
    let hi = mix(B.a0, B.a1, open) + wob * 0.9 + ripple * open;
    let fade = sat(open * 1.4 - fi * 0.08);
    if (fade <= 0.0) { return; }
    if (ang >= hi && ang <= lo) { (*st).top = i; (*st).lo = lo; (*st).hi = hi; (*st).f = fade; }
    let fl = select(0.0, g.flareK, i == g.fb);

    // ---- bordi al neon con impulsi che corrono ----
    // test economico prima di seno e coseno: |sin x| >= 2|x|/π
    let dDeg = ang - hi;
    if (abs(dDeg) < 90.0 && r * abs(radians(dDeg)) * 0.6366 <= 60.0 * k) {
        let dA = radians(dDeg);
        let along = r * cos(dA);
        let d = abs(r * sin(dA));
        if (along > 0.0 && d <= 60.0 * k) {
            let tt = along / L;
            var inten = select(mix(0.25 + fl * 0.4, 0.2 + fl * 0.3, (tt - 0.16) / 0.84), (0.25 + fl * 0.4) * tt / 0.16, tt < 0.16);
            // due impulsi, srotolati
            let i1 = impulso(tt, modp(t * 0.17 + B.ph * 0.23, 1.5) - 0.25);
            let i2 = impulso(tt, modp(t * 0.11 + B.ph * 0.23 + 0.7, 1.5) - 0.25);
            let hot = max(i1.x, i2.x);
            inten += i1.y + i2.y;
            inten = (inten + hot * 0.6) * fade;
            let c = mix(B.edge, vec3f(1.0), 0.65 * hot);
            let prof = stroke(d, 12.0) * (0.08 + fl * 0.08) + stroke(d, 4.0) * 0.2 + stroke(d, 1.3);
            (*st).acc += c * inten * prof;
        }
    }

    // ---- polvere luminosa che sale lungo le lame ----
    // celle radiali che scorrono verso l'esterno, due strati a velocità diverse
    if (open > 0.6 && g.rr > 0.1 && ang >= hi - g.mdust && ang <= lo + g.mdust) {
        let vj = 0.85 + 0.3 * fract(B.ph * 0.618);
        (*st).acc += polvere(i, 1u, 0.035 * vj, 0.0525 * vj, g.rr, r, ang, lo, hi, L, k, t, B.edge);
        (*st).acc += polvere(i, 2u, 0.06 * vj, 0.09 * vj, g.rr, r, ang, lo, hi, L, k, t, B.edge);
    }
}

fn filamento(j: i32, col: ptr<function, vec3f>, p: vec2f, P: vec2f, r: f32, ang: f32, L: f32, k: f32, t: f32, open: f32) {
    let fj = f32(j);
    let F = filo(j);
    let aDeg = F.a + 2.0 * sin(t * 0.2 + fj * 1.7);
    let s = modp(t * F.sp + fj * 0.37, 1.3) - 0.15;
    let dDeg = ang - aDeg;
    if (abs(dDeg) < 90.0 && r * abs(radians(dDeg)) * 0.6366 <= 18.0 * k
        && r >= L * s - 18.0 * k && r <= L * (s + 0.3) + 18.0 * k) {
        let an = radians(aDeg);
        let dir = vec2f(cos(an), sin(an));
        let nrm = vec2f(-dir.y, dir.x) * 14.0 * k;
        let sg = seg(p, P + dir * L * s + nrm, P + dir * L * (s + 0.3) + nrm);
        if (sg.x < 3.0 * k) {
            let g = select(mix(1.0, 0.0, (sg.y - 0.6) / 0.4), sg.y / 0.6, sg.y < 0.6);
            *col += F.col * F.al * open * g * stroke(sg.x, 1.2);
        }
    }
}

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let open = ease_out((u.since - 0.2) / 2.6);

    // ---- sfondo: nero inchiostro con due luci che si muovono ----
    var col = rgb(4.0, 3.0, 12.0);
    let g1 = vec2f(W * (0.74 + 0.06 * sin(t * 0.07)), H * (0.2 + 0.07 * cos(t * 0.09)));
    let d1 = length(p - g1) / (max(W, H) * 0.75);
    let pulse = 0.8 + 0.2 * sin(t * 0.5);
    let gc = mix(rgb(40.0,48.0,190.0), rgb(26.0,20.0,110.0), sat(d1 / 0.45));
    let ga = select(mix(0.18, 0.0, sat((d1 - 0.45) / 0.55)), mix(0.45 * pulse, 0.18, d1 / 0.45), d1 < 0.45);
    col = mix(col, gc, ga);
    let g2 = vec2f(W * (0.9 + 0.05 * cos(t * 0.06)), H * (0.88 + 0.06 * sin(t * 0.08)));
    let d2 = length(p - g2) / (max(W, H) * 0.55);
    col = mix(col, rgb(120.0,24.0,170.0), (0.28 + 0.08 * sin(t * 0.43 + 1.0)) * sat(1.0 - d2));

    // ---- geometria del ventaglio ----
    let P = vec2f(W * (0.30 + 0.03 * sin(t * 0.11)), H * (1.06 + 0.02 * cos(t * 0.09)));
    let L = length(vec2f(W - P.x, P.y)) * 1.15;
    let v = p - P;
    let r = length(v);
    let rr = r / L;
    let ang = degrees(atan2(v.y, v.x));

    // lampo periodico su una lama a caso
    var fb = -1;
    var flareK = 0.0;
    if (t > 4.0 && u.since > 3.0) {
        let fc = floor((t - 4.0) / 7.5);
        fb = i32(floor(hash11(fc) * 6.0));
        flareK = sin(min(1.0, ((t - 4.0) - fc * 7.5) / 2.2) * PI);
    }

    // margine angolare (gradi) che copre il raggio massimo di una particella di polvere
    let g = Geo(r, r / L, ang, L, k, t, open, flareK, fb, degrees(5.8 * k / max(r, 1.0)));

    // Un giro sulle lame (srotolato a mano): trova la lama più in alto e intanto accumula
    // bordi al neon e polvere (sono solo additivi, si sommano dopo il corpo).
    var st = Stato(-1, 0.0, 0.0, 0.0, vec3f(0.0));
    lamina(0, &st, g);
    lamina(1, &st, g);
    lamina(2, &st, g);
    lamina(3, &st, g);
    lamina(4, &st, g);
    lamina(5, &st, g);
    let top = st.top;
    let tLo = st.lo;
    let tHi = st.hi;
    let tF = st.f;
    let acc = st.acc;

    // ---- corpo della lama più in alto in questo pixel ----
    if (top >= 0) {
        let B = lama(top);
        let fl = select(0.0, flareK, top == fb);
        let f = tF;
        let mid = (tLo + tHi) * 0.5;
        let tt = r * cos(radians(ang - mid)) / L;
        let dark = rgb(6.0, 5.0, 20.0);
        var c: vec3f;
        if (tt < 0.25) { c = mix(dark, B.body, 0.22 + fl * 0.2); }
        else if (tt < 0.6) { c = mix(mix(dark, B.body, 0.22 + fl * 0.2), mix(dark, B.body, 0.55 + fl * 0.3), (tt - 0.25) / 0.35); }
        else { c = mix(mix(dark, B.body, 0.55 + fl * 0.3), mix(dark, B.body, 0.12), sat((tt - 0.6) / 0.4)); }
        let a = ramp4(tt, 0.0, 0.92 * f, 0.95 * f, 0.95 * f, 0.25, 0.6);
        col = mix(col, c, a);
        // ombra lungo il bordo inferiore: stacca i piani
        let dlo = r * sin(radians(tLo - ang));
        col = mix(col, rgb(2.0, 1.0, 8.0), 0.55 * f * sat(1.0 - dlo / (90.0 * k)));
        // lucentezza sulla parte alta
        let sheenA = tHi + (tLo - tHi) * 0.42;
        if (ang < sheenA) {
            // due riflessi che scorrono, srotolati
            let prof = 0.36 * riflesso(tt, modp(t * 0.11 + B.ph * 0.17, 1.7) - 0.35)
                     + 0.22 * riflesso(tt, modp(t * 0.07 + B.ph * 0.17 + 0.8, 1.7) - 0.35);
            col += B.edge * B.sheen * f * prof;
            var add: vec3f;
            if (tt < 0.45) { add = B.body * (0.10 + fl * 0.25) * sat(tt / 0.45); }
            else if (tt < 0.7) { add = mix(B.body * (0.10 + fl * 0.25), B.edge * (0.22 + fl * 0.35), (tt - 0.45) / 0.25); }
            else { add = mix(B.edge * (0.22 + fl * 0.35), B.body * 0.04, sat((tt - 0.7) / 0.3)); }
            col += add * B.sheen * f;
        }
    }
    col += acc;

    // ---- filamenti sottili che scivolano lungo il ventaglio ----
    // srotolati a mano, con un test economico in coordinate polari prima del segmento vero
    filamento(0, &col, p, P, r, ang, L, k, t, open);
    filamento(1, &col, p, P, r, ang, L, k, t, open);
    filamento(2, &col, p, P, r, ang, L, k, t, open);
    filamento(3, &col, p, P, r, ang, L, k, t, open);
    filamento(4, &col, p, P, r, ang, L, k, t, open);

    // bagliore pulsante all'origine del ventaglio
    let pr = min(W, H) * (0.32 + 0.05 * sin(t * 0.8));
    col += rgb(120.0, 80.0, 255.0) * 0.15 * open * sat(1.0 - r / pr);

    // lato sinistro più scuro, dove sta il logo
    let xr = p.x / (0.55 * W);
    let va = select(select(0.0, mix(0.35, 0.0, (xr - 0.6) / 0.4), xr < 1.0), mix(0.85, 0.35, xr / 0.6), xr < 0.6);
    col = mix(col, rgb(3.0, 2.0, 10.0), va);
    // vignettatura
    let dv = length(p - u.canvas * 0.5);
    let r0 = min(W, H) * 0.4;
    let r1 = length(u.canvas) * 0.62;
    col *= 1.0 - 0.5 * sat((dv - r0) / (r1 - r0));
    return col;
}
