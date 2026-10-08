// nome: Geometrie
// Rombi concentrici con gli angoli del marchio, chevron < >, triangoli, esagoni, orbita e pannelli di vetro.

fn logo_layout() -> vec3f {
    let w = min(0.26 * u.canvas.x, 0.46 * u.canvas.y) / 0.9;
    return vec3f(0.09 * u.canvas.x + w * 0.5, 0.5 * u.canvas.y, w);
}

// Distanza dal contorno di un poligono (n vertici) e posizione lungo il perimetro (0..1).
fn poly_d(p: vec2f, pts: array<vec2f, 6>, n: i32, closed: bool) -> vec2f {
    var v = pts;
    let ns = select(n - 1, n, closed);
    var total = 0.0;
    for (var i = 0; i < ns; i++) { total += length(v[(i + 1) % n] - v[i]); }
    var best = 1e9;
    var at = 0.0;
    var acc = 0.0;
    for (var i = 0; i < ns; i++) {
        let a = v[i];
        let b = v[(i + 1) % n];
        let s = seg(p, a, b);
        let l = length(b - a);
        if (s.x < best) { best = s.x; at = (acc + s.y * l) / total; }
        acc += l;
    }
    return vec2f(best, at);
}

fn inside(p: vec2f, pts: array<vec2f, 6>, n: i32) -> bool {
    var v = pts;
    var pos = 0;
    var neg = 0;
    for (var i = 0; i < n; i++) {
        let a = v[i];
        let b = v[(i + 1) % n];
        let c = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if (c > 0.0) { pos++; } else { neg++; }
    }
    return pos == n || neg == n;
}

fn place(src: array<vec2f, 6>, n: i32, c: vec2f, s: f32, r: f32) -> array<vec2f, 6> {
    var v = src;
    var o: array<vec2f, 6>;
    let m = rot2(r);
    for (var i = 0; i < n; i++) { o[i] = c + m * v[i] * s; }
    return o;
}

// Riempimento "vetro": sfumatura diagonale molto trasparente, sommata alla luce sotto.
fn glass(p: vec2f, pts: array<vec2f, 6>, n: i32, col: vec3f, a: f32, ang: f32) -> vec3f {
    if (!inside(p, pts, n)) { return vec3f(0.0); }
    var v = pts;
    var lo = vec2f(1e9);
    var hi = vec2f(-1e9);
    for (var i = 0; i < n; i++) { lo = min(lo, v[i]); hi = max(hi, v[i]); }
    let c = (lo + hi) * 0.5;
    let r = max(hi.x - lo.x, hi.y - lo.y) * 0.5;
    let s = sat((dot(p - c, vec2f(cos(ang), sin(ang))) / r + 1.0) * 0.5);
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

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let A = ease_out((u.since - 0.2) / 2.6);
    let CX = 0.73 * W;
    let CY = 0.5 * H;

    let SKY = rgb(112.0,189.0,232.0);
    let AZ = rgb(49.0,140.0,211.0);
    let IND = rgb(90.0,100.0,240.0);
    let VI = rgb(134.0,18.0,173.0);
    let LI = rgb(194.0,136.0,214.0);
    let MAG = rgb(200.0,110.0,240.0);

    // sfondo e luci
    var col = rgb(4.0, 3.0, 12.0);
    let g1 = vec2f(W * (0.74 + 0.05 * sin(t * 0.07)), H * (0.3 + 0.08 * cos(t * 0.09)));
    let d1 = length(p - g1) / (max(W, H) * 0.7);
    col = mix(col, select(rgb(26.0,20.0,110.0), mix(rgb(40.0,48.0,190.0), rgb(26.0,20.0,110.0), d1 / 0.5), d1 < 0.5),
              select(mix(0.14, 0.0, sat((d1 - 0.5) / 0.5)), mix(0.38 + 0.08 * sin(t * 0.5), 0.14, d1 / 0.5), d1 < 0.5));
    let d2 = length(p - vec2f(0.9 * W, 0.95 * H)) / (max(W, H) * 0.55);
    col = mix(col, rgb(120.0,24.0,170.0), (0.26 + 0.07 * sin(t * 0.43 + 1.0)) * sat(1.0 - d2));

    // lampo periodico
    var fid = -1;
    var fk = 0.0;
    if (t > 4.0 && u.since > 3.0) {
        let fc = floor((t - 4.0) / 7.5);
        fid = i32(floor(hash11(fc + 9.0) * 6.0));
        fk = sin(min(1.0, ((t - 4.0) - fc * 7.5) / 2.0) * PI);
    }

    // 1) grandi piani scuri in diagonale
    for (var j = 0; j < 2; j++) {
        let fj = f32(j);
        let c = select(AZ, VI, j == 0);
        let ctr = select(vec2f(0.66 * W, 1.0 * H), vec2f(0.9 * W, 0.62 * H), j == 0) + vec2f(sin(t * 0.05 + fj) * H * 0.02, 0.0);
        let r = -0.87 + 0.015 * sin(t * 0.07 + fj);
        let l = select(3.0, 3.2, j == 0) * H;
        let w = select(0.16, 0.3, j == 0) * H;
        let lp = rot2(-r) * (p - ctr);
        if (abs(lp.x) < l * 0.5 && abs(lp.y) < w * 0.5) {
            let s = (lp.y + w * 0.5) / w;
            col = mix(col, mix(mix(rgb(8.0,6.0,24.0), c, 0.2), rgb(5.0,4.0,14.0), s), 0.85 * A);
        }
        if (abs(lp.x) < l * 0.5) {
            let d = abs(lp.y + w * 0.5);
            let ec = mix(c, vec3f(1.0), 0.2);
            col += ec * outline(d, 1.3) * 0.35 * A;
            col += pulse(d, (lp.x + l * 0.5) / l, ((t * 0.08 + fj * 0.5) % 1.4) - 0.1, 0.18, false, mix(c, vec3f(1.0), 0.3)) * A;
        }
    }

    // 2) pannelli di vetro che scorrono a ~50°, come i bracci del logo
    let pr = -0.87;
    var pcol = array<vec3f, 7>(AZ, IND, VI, LI, SKY, rgb(85.0,0.0,124.0), MAG);
    for (var j = 0; j < 7; j++) {
        let fj = f32(j);
        let sp = 0.012 + 0.02 * hash11(fj + 20.0);
        let uu = (hash11(fj + 30.0) * 1.4 + sp * t) % 1.4;
        let vv = (fj / 6.0) * 1.2 - 0.1;
        let l = (0.35 + 0.5 * hash11(fj + 40.0)) * H;
        let w = (0.025 + 0.05 * hash11(fj + 50.0)) * H;
        let al = 0.06 + 0.09 * hash11(fj + 60.0);
        let along = (uu - 0.2) * length(u.canvas) * 0.9;
        let across = (vv - 0.5) * H * 1.1;
        let ctr = vec2f(0.78 * W, 0.5 * H) + rot2(pr) * vec2f(along, across);
        let fa = A * min(1.0, sin(PI * sat(uu / 1.4)) * 2.0);
        let lp = rot2(-pr) * (p - ctr);
        if (abs(lp.x) < l * 0.5) {
            if (abs(lp.y) < w * 0.5) {
                let s = (lp.y + w * 0.5) / w;
                let dark = rgb(6.0, 5.0, 20.0);
                col += select(mix(mix(pcol[j], dark, 0.6) * 0.5, pcol[j] * 0.08, (s - 0.55) / 0.45), mix(pcol[j], mix(pcol[j], dark, 0.6) * 0.5, s / 0.55), s < 0.55) * al * fa;
            }
            col += mix(pcol[j], vec3f(1.0), 0.25) * outline(abs(lp.y + w * 0.5), 1.0) * 0.45 * fa;
        }
    }

    // 3) orbita tratteggiata con satellite
    let orR = 0.41 * H;
    let ep = rot2(0.35) * (p - vec2f(CX, CY));
    let er = length(ep / vec2f(orR, orR * 0.92));
    let ed = abs(er - 1.0) * orR * 0.96;
    let arc = (atan2(ep.y / 0.92, ep.x) + PI) * orR;
    let dash = step(fract((arc + t * 20.0 * k) / (12.0 * k)), 2.0 / 12.0);
    col += LI * 0.28 * A * dash * stroke(ed, 1.2);
    let sa = t * 0.35;
    let sat_p = vec2f(CX, CY) + rot2(-0.35) * vec2f(cos(sa) * orR, sin(sa) * orR * 0.92);
    let sdd = length(p - sat_p) / (9.0 * k);
    if (sdd < 1.0) { col += select(SKY * 0.5 * (1.0 - (sdd - 0.4) / 0.6), mix(vec3f(0.9), SKY * 0.5, sdd / 0.4), sdd < 0.4) * A; }

    // 4) rombi concentrici
    var RH = array<vec2f, 6>(vec2f(0.0, -1.0), vec2f(0.84, 0.0), vec2f(0.0, 1.0), vec2f(-0.84, 0.0), vec2f(0.0), vec2f(0.0));
    var rs = array<f32, 4>(0.30, 0.235, 0.17, 0.105);
    var rr = array<f32, 4>(0.06 * sin(t * 0.12), -0.09 * sin(t * 0.15 + 1.0), 0.14 * sin(t * 0.1 + 2.0), -0.2 * sin(t * 0.18));
    var rc = array<vec3f, 4>(AZ, IND, VI, LI);
    let br = 1.0 + 0.035 * sin(t * 0.6);
    for (var j = 0; j < 4; j++) {
        let fj = f32(j);
        let pts = place(RH, 4, vec2f(CX, CY), H * rs[j] * br * mix(0.6, 1.0, A), rr[j]);
        if (j == 1 || j == 3) { col += glass(p, pts, 4, rc[j], 0.16 * A, -0.9); }
        let f = select(0.0, fk, fid == j);
        let pd = poly_d(p, pts, 4, true);
        if (pd.x < 40.0 * k) {
            col += mix(rc[j], vec3f(1.0), 0.15 + f * 0.4) * outline(pd.x, 1.3) * (0.42 + f * 0.5) * A;
            let sgn = select(1.0, -1.0, j % 2 == 1);
            col += pulse(pd.x, pd.y, t * (0.06 + fj * 0.025) * sgn + fj * 0.3, 0.16, true, mix(rc[j], LI, 0.3)) * A;
        }
    }

    // 5) chevron < > ai lati dei rombi
    var CH = array<vec2f, 6>(vec2f(0.45, -1.0), vec2f(-0.45, 0.0), vec2f(0.45, 1.0), vec2f(0.0), vec2f(0.0), vec2f(0.0));
    let gap = H * (0.33 + 0.025 * sin(t * 0.8));
    for (var j = 0; j < 2; j++) {
        let sgn = select(1.0, -1.0, j == 0);
        let c = select(SKY, VI, j == 0);
        var pts = place(CH, 3, vec2f(0.0), H * 0.075, 0.0);
        for (var m = 0; m < 3; m++) { pts[m] = vec2f(CX + sgn * gap, CY) + vec2f(-pts[m].x * sgn, pts[m].y); }
        let pd = poly_d(p, pts, 3, false);
        if (pd.x < 40.0 * k) {
            let f = select(0.0, fk, fid == 4);
            col += c * outline(pd.x, 2.0) * (0.55 + f * 0.4) * A;
            col += pulse(pd.x, pd.y, ((t * 0.25 + f32(j) * 0.5) % 1.6) - 0.3, 0.35, false, mix(c, vec3f(1.0), 0.3)) * A;
        }
    }

    // 6) triangoli che ruotano lenti
    var TR = array<vec2f, 6>(vec2f(0.0, -1.0), vec2f(0.866, 0.5), vec2f(-0.866, 0.5), vec2f(0.0), vec2f(0.0), vec2f(0.0));
    var tp = array<vec4f, 4>(vec4f(0.585, 0.2, 0.065, 0.15), vec4f(0.94, 0.78, 0.1, -0.1), vec4f(0.62, 0.84, 0.045, 0.22), vec4f(0.97, 0.12, 0.04, -0.25));
    var tc = array<vec3f, 4>(SKY, MAG, LI, IND);
    for (var j = 0; j < 4; j++) {
        let fj = f32(j);
        let q = tp[j];
        let ctr = vec2f(W * q.x, H * q.y) + vec2f(sin(t * 0.13 + fj), cos(t * 0.11 + fj)) * H * 0.015;
        let pts = place(TR, 3, ctr, H * q.z, q.w * t * 0.4 + fj);
        col += glass(p, pts, 3, tc[j], 0.12 * A, -0.9);
        let pd = poly_d(p, pts, 3, true);
        if (pd.x < 40.0 * k) {
            let f = select(0.0, fk, fid == 5);
            col += tc[j] * outline(pd.x, 1.3) * (0.5 + f * 0.4) * A;
            col += pulse(pd.x, pd.y, t * 0.12 + fj * 0.25, 0.2, true, tc[j]) * A;
        }
    }

    // 7) esagono doppio con nodi
    var HX: array<vec2f, 6>;
    for (var m = 0; m < 6; m++) { HX[m] = vec2f(cos(f32(m) * PI / 3.0), sin(f32(m) * PI / 3.0)); }
    let hc = vec2f(0.915 * W, 0.24 * H);
    let hp = place(HX, 6, hc, H * 0.085, t * 0.05);
    let hd = poly_d(p, hp, 6, true);
    col += IND * outline(hd.x, 1.3) * 0.4 * A;
    col += pulse(hd.x, hd.y, -t * 0.1, 0.18, true, SKY) * A;
    let hp2 = place(HX, 6, hc, H * 0.05, -t * 0.08);
    col += MAG * outline(poly_d(p, hp2, 6, true).x, 1.0) * 0.3 * A;
    for (var m = 0; m < 6; m++) {
        let a = (0.4 + 0.6 * max(0.0, sin(t * 1.2 - f32(m) * 1.05))) * A;
        let dn = length(p - hp[m]) / (6.0 * k);
        if (dn < 1.0) { col += select(SKY * 0.5 * a * (1.0 - (dn - 0.4) / 0.6), mix(vec3f(a), SKY * 0.5 * a, dn / 0.4), dn < 0.4); }
    }

    // 8) griglia di punti a rombo
    let gp = (p - vec2f(0.62 * W, 0.62 * H)) / H;
    let gj = round(gp.y / 0.042);
    let gi = round((gp.x - select(0.0, 0.025, i32(gj) % 2 == 1)) / 0.05);
    if (gi >= 0.0 && gi < 6.0 && gj >= 0.0 && gj < 4.0) {
        let cdot = vec2f(0.62 * W + gi * 0.05 * H + select(0.0, 0.025 * H, i32(gj) % 2 == 1), 0.62 * H + gj * 0.042 * H);
        let dd = abs(p - cdot);
        if (dd.x + dd.y < 1.6 * k) { col += LI * (0.12 + 0.25 * max(0.0, sin(t * 0.9 - gi * 0.5 - gj * 0.7))) * A; }
    }

    // 9) polvere luminosa che sale
    if (A > 0.5) {
        var dc = array<vec3f, 4>(SKY, LI, MAG, IND);
        for (var n = 0; n < 50; n++) {
            let fn_ = f32(n);
            let pd = 6.0 + 8.0 * hash11(fn_);
            let tt0 = t + hash11(fn_ + 0.3) * pd;
            let cyc = floor(tt0 / pd);
            let lt = tt0 - cyc * pd;
            let sd = fn_ * 7.3 + cyc * 3.1;
            let pos = vec2f((0.45 + 0.6 * hash11(sd)) * W + (hash11(sd + 1.0) - 0.3) * 0.004 * lt * W,
                            (0.2 + 0.85 * hash11(sd + 2.0)) * H - (0.006 + 0.014 * hash11(sd + 3.0)) * lt * H);
            let rad = (0.6 + 1.5 * hash11(sd + 4.0)) * 2.6 * k;
            let d = length(p - pos);
            if (d < rad) {
                let f = min(1.0, lt / 1.5) * sat((pd - lt) / 1.5) * (0.55 + 0.45 * sin(t * 2.0 + fn_));
                let c = dc[n % 4];
                let x = d / rad;
                col += select(c * 0.3 * (1.0 - (x - 0.4) / 0.6), mix(vec3f(0.65), c * 0.3, x / 0.4), x < 0.4) * f;
            }
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
