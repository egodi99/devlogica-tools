// nome: Quiete
// tipo: fisso
// Composizione minimale: un grande chevron ">" fatto di fili al neon concentrici e una lama di vetro a forma di "/".

fn logo_layout() -> vec3f {
    let w = min(0.26 * u.canvas.x, 0.46 * u.canvas.y) / 0.9;
    return vec3f(0.09 * u.canvas.x + w * 0.5, 0.5 * u.canvas.y, w);
}

fn qu_cross(a: vec2f, b: vec2f) -> f32 { return a.x * b.y - a.y * b.x; }

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let vm = max(W, H);

    let SKY = rgb(112.0,189.0,232.0);
    let CY = rgb(86.0,214.0,246.0);
    let AZ = rgb(49.0,140.0,211.0);
    let IND = rgb(90.0,100.0,240.0);
    let VI = rgb(134.0,18.0,173.0);
    let LI = rgb(194.0,136.0,214.0);
    let MAG = rgb(200.0,110.0,240.0);

    // ---- sfondo: gradiente profondo blu notte → inchiostro, due luci morbide ----
    let s0 = sat(p.x / W * 0.7 + p.y / H * 0.3);
    var col = mix(rgb(5.0, 4.0, 14.0), rgb(10.0, 10.0, 34.0), s0);
    col = mix(col, rgb(36.0, 40.0, 150.0), 0.30 * glow(length(p - vec2f(0.80 * W, 0.42 * H)), 0.42 * vm));
    col = mix(col, rgb(110.0, 20.0, 150.0), 0.22 * glow(length(p - vec2f(0.66 * W, 1.02 * H)), 0.38 * vm));

    // ---- lama di vetro "/" dietro al chevron ----
    let sa = radians(50.0);
    let sdir = vec2f(cos(sa), -sin(sa));          // verso l'alto a destra
    let snrm = vec2f(sin(sa), cos(sa));
    let sc = vec2f(0.60 * W, 0.5 * H);
    let sl = p - sc;
    let sw = 0.085 * H;
    let sx = dot(sl, snrm);                       // distanza trasversale (con segno)
    let sy = dot(sl, sdir) / (0.75 * H);          // posizione lungo la lama
    let sfade = sat(1.0 - abs(sy) / 1.0);
    if (abs(sx) < sw) {
        let t = (sx + sw) / (2.0 * sw);
        let gc = mix(LI, IND, sat(0.5 - sy * 0.5));
        col += gc * mix(0.14, 0.02, t) * sfade * sfade;
    }
    col += mix(LI, SKY, sat(0.5 - sy * 0.5)) * 0.35 * sfade * sfade * (stroke(abs(sx + sw), 1.0) + 0.08 * exp(-abs(sx + sw) / (8.0 * k)));
    col += LI * 0.10 * sfade * sfade * stroke(abs(sx - sw), 0.8);

    // ---- grande chevron ">" con fili concentrici ----
    let ca = radians(50.0);
    let apex = vec2f(0.86 * W, 0.5 * H);
    var q = p - apex;
    q.y = abs(q.y);
    let d = vec2f(-cos(ca), sin(ca));             // braccio verso il basso a sinistra (con |y|)
    let sgn = qu_cross(d, q);                     // > 0 dentro il chevron (verso sinistra)
    let along = dot(q, d);
    let sp = 15.0 * k;                            // passo tra i fili
    let fi = round(sgn / sp);
    let dl = abs(sgn - fi * sp);
    let NL = 9.0;
    if (fi >= 0.0 && fi < NL) {
        let x = fi / (NL - 1.0);
        // dal filo esterno ciano al più interno viola
        let lc = select(mix(IND, MAG, (x - 0.5) / 0.5), mix(CY, IND, x / 0.5), x < 0.5);
        let len = 1.25 * H;
        let tt = along / len;
        let tip = fi * sp / sin(ca) * cos(ca);
        if (along > -tip - sp && tt < 1.0) {
            let fade = sat(1.0 - tt) * sat(1.0 - tt) * mix(1.0, 0.35, x);
            let w = select(0.8, 1.6, fi == 0.0 || fi == 3.0);
            col += mix(lc, vec3f(1.0), select(0.0, 0.25, fi == 0.0)) * fade * (stroke(dl, w) * 0.8 + 0.06 * exp(-dl / (4.0 * k)));
        }
    }
    // vetro tra il filo esterno e il quarto: sfumatura che si spegne verso l'interno
    if (sgn > 0.0 && sgn < 3.0 * sp && along < 1.25 * H) {
        let t = sgn / (3.0 * sp);
        let tt = sat(along / (1.25 * H));
        col += mix(AZ, IND, t) * mix(0.11, 0.02, t) * (1.0 - tt) * (1.0 - tt);
    }
    // bagliore sul vertice
    col += mix(CY, SKY, 0.5) * 0.18 * glow(length(p - apex), 0.10 * H);
    col += vec3f(1.0) * 0.5 * glow(length(p - apex), 3.0 * k);

    // ---- punti di polvere fissi, radi, lungo la diagonale ----
    let G = 46.0 * k;
    let cell = floor(p / G);
    let hr = hash21(cell + 0.31);
    let ofs = vec2f(hash21(cell + 4.7), hash21(cell + 9.1));
    let dc = length(p - (cell + 0.15 + 0.7 * ofs) * G);
    let band = glow(dot(p - sc, snrm) / H, 0.35) * sat(p.x / W * 2.0 - 0.6);
    if (hr > 0.93) { col += mix(LI, SKY, ofs.x) * 0.5 * band * glow(dc, (0.8 + 1.2 * ofs.y) * k); }

    // lato sinistro più scuro, dove sta il logo
    let xr = p.x / (0.55 * W);
    let va = select(select(0.0, mix(0.3, 0.0, (xr - 0.6) / 0.4), xr < 1.0), mix(0.8, 0.3, xr / 0.6), xr < 0.6);
    col = mix(col, rgb(3.0, 2.0, 10.0), va);
    // vignettatura
    let dv = length(p - u.canvas * 0.5);
    let r0 = min(W, H) * 0.4;
    let r1 = length(u.canvas) * 0.62;
    col *= 1.0 - 0.5 * sat((dv - r0) / (r1 - r0));
    return col;
}
