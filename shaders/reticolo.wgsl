// nome: Reticolo
// Reticolo di rombi con impulsi di dati che scorrono e una fascia di luce a forma di "/".

fn logo_layout() -> vec3f {
    let w = min(0.30 * u.canvas.x, 0.533 * u.canvas.y) / 0.9;
    return u.canvas.xyx * vec3f(0.5, 0.5, 0.0) + vec3f(0.0, 0.0, w);
}

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let C = u.canvas * 0.5;

    // sfondo: viola profondo a sinistra, blu notte a destra
    let s = sat((p.x / W * 0.8 + p.y / H * 0.45) / 1.25);
    var col = select(mix(rgb(14.0,16.0,48.0), rgb(7.0,26.0,46.0), (s - 0.52) / 0.48), mix(rgb(23.0,10.0,44.0), rgb(14.0,16.0,48.0), s / 0.52), s < 0.52);
    let vm = max(W, H);
    col = mix(col, rgb(134.0,18.0,173.0), 0.20 * sat(1.0 - length((p - vec2f(0.28 * W, 0.46 * H)) / vec2f(0.6 * vm, 0.45 * vm)) / 0.7));
    col = mix(col, rgb(49.0,140.0,211.0), 0.15 * sat(1.0 - length((p - vec2f(0.74 * W, 0.58 * H)) / vec2f(0.55 * vm, 0.4 * vm)) / 0.7));

    // coordinate del reticolo: assi inclinati di ±50°, come i bracci di < e >
    let L = clamp(W / 24.0, 56.0 * k, 96.0 * k);
    let ca = cos(radians(50.0));
    let sa = sin(radians(50.0));
    let a = vec2f(ca, -sa) * L;
    let b = vec2f(ca, sa) * L;
    let d = p - C;
    let det = a.x * b.y - a.y * b.x;
    let gi = (d.x * b.y - d.y * b.x) / det;
    let gj = (a.x * d.y - a.y * d.x) / det;
    let sp = L * sin(radians(100.0));
    let dA = abs(fract(gj + 0.5) - 0.5) * sp;
    let dB = abs(fract(gi + 0.5) - 0.5) * sp;
    let node = C + round(gi) * a + round(gj) * b;
    let dn = abs(p - node);
    let isNode = (dn.x + dn.y) < 1.6 * k;
    let lineC = mix(rgb(194.0,136.0,214.0), rgb(112.0,189.0,232.0), sat(p.x / W));
    let lineCov = max(stroke(dA, 0.8), stroke(dB, 0.8));

    // fascia di luce che attraversa lo schermo inclinata come la barra "/"
    var band = 0.0;
    var glowB = 0.0;
    let ph = (t - 3.2) % 26.0;
    if (t > 3.2 && ph < 10.0) {
        let e = ph / 10.0;
        let kk = (1.0 - cos(PI * e)) * 0.5 * 0.35 + e * 0.65;
        let N = normalize(vec2f(75.0, 28.0));
        let pos = mix(-300.0 * k, W * N.x + H * N.y + 300.0 * k, kk);
        let db = abs(dot(p, N) - pos);
        band = step(db, 162.0 * k);
        glowB = sat(1.0 - db / (288.0 * k));
    }
    col += lineC * lineCov * mix(0.055, 0.22 * 0.75, band);
    if (isNode) { col = mix(col, rgb(241.0,234.0,247.0), mix(0.16, 0.5 * 0.75, band)); }
    col += rgb(194.0,136.0,214.0) * 0.09 * glowB;

    // impulsi che viaggiano lungo le linee del reticolo
    let R = length(u.canvas) / (2.0 * L);
    for (var n = 0; n < 16; n++) {
        let fn_ = f32(n);
        let pd = 9.0 + 6.0 * hash11(fn_ + 1.0);
        let tt = t + hash11(fn_) * 20.0;
        let cyc = floor(tt / pd);
        let lt = tt - cyc * pd;
        let sd = fn_ * 17.0 + cyc;
        let famB = hash11(sd) < 0.5;
        let J = round((hash11(sd + 1.0) - 0.5) * R * 1.4);
        let i0 = (hash11(sd + 2.0) - 0.5) * R * 1.4;
        let dir = select(-1.0, 1.0, hash11(sd + 3.0) < 0.5);
        let speed = (55.0 + 70.0 * hash11(sd + 4.0)) * k / L;
        let tail = (110.0 + 120.0 * hash11(sd + 5.0)) * k / L;
        let head = i0 + dir * speed * lt;
        let along_px = select(gi, gj, famB);
        let across = select(gj, gi, famB);
        let back = dir * (head - along_px);
        let perp = abs(across - J) * sp;
        let fade = min(1.0, lt / 1.2) * sat((pd - lt) / 1.5);
        let c = mix(rgb(160.0,70.0,200.0), rgb(90.0,170.0,232.0), sat(p.x / W));
        if (back >= 0.0 && back <= tail && perp < 6.0 * k) {
            let f = 1.0 - back / tail;
            col += mix(c, vec3f(1.0), f * 0.5) * pow(f, 1.6) * fade * stroke(perp, 0.8 + 1.6 * f);
        }
        let hd = length(vec2f((along_px - head) * L, perp));
        col += mix(c, vec3f(1.0), 0.6) * 0.8 * fade * glow(hd, 4.0 * k);
    }

    // alone scuro dietro al logo
    let e = length((p - C) / vec2f(0.31 * W, 0.17 * W));
    let ha = select(select(0.0, mix(0.45, 0.0, (e - 0.55) / 0.45), e < 1.0), mix(0.82, 0.45, e / 0.55), e < 0.55);
    col = mix(col, rgb(14.0, 8.0, 34.0), ha * u.logo_on);
    // vignettatura
    let dv = length((p - C) / vec2f(1.2 * vm, 0.8 * vm));
    col = mix(col, rgb(3.0, 2.0, 10.0), 0.55 * sat((dv - 0.45) / 0.55));
    return col;
}
