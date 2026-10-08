// nome: Barre diagonali
// Barre a 45° sovrapposte con ombre, strisce al neon, tratteggi e riflessi che scorrono.

fn logo_layout() -> vec3f {
    let w = min(0.23 * u.canvas.x, 0.41 * u.canvas.y) / 0.9;
    return vec3f(0.74 * u.canvas.x, 0.73 * u.canvas.y, w);
}

struct Pal { a: vec3f, b: vec3f, sh: f32, kind: i32 }; // kind: 0 pieno, 1 neon, 2 filo, 3 tratteggio, 4 linee

fn pal(ty: i32) -> Pal {
    switch ty {
        case 0: { return Pal(rgb(44.0,58.0,88.0), rgb(16.0,20.0,36.0), 0.6, 0); }
        case 1: { return Pal(rgb(38.0,50.0,80.0), rgb(14.0,18.0,32.0), 0.6, 0); }
        case 2: { return Pal(rgb(62.0,22.0,84.0), rgb(16.0,8.0,26.0), 0.55, 0); }
        case 3: { return Pal(rgb(86.0,214.0,246.0), rgb(20.0,70.0,130.0), 0.5, 1); }
        case 4: { return Pal(rgb(86.0,214.0,246.0), rgb(49.0,140.0,211.0), 0.5, 1); }
        case 5: { return Pal(rgb(150.0,235.0,255.0), rgb(86.0,214.0,246.0), 0.4, 1); }
        case 6: { return Pal(rgb(205.0,90.0,245.0), rgb(134.0,18.0,173.0), 0.5, 1); }
        case 7: { return Pal(rgb(120.0,40.0,160.0), rgb(70.0,18.0,100.0), 0.5, 3); }
        case 8: { return Pal(rgb(70.0,110.0,170.0), rgb(20.0,26.0,46.0), 0.4, 4); }
        case 9: { return Pal(rgb(86.0,214.0,246.0), rgb(86.0,214.0,246.0), 0.0, 2); }
        default: { return Pal(rgb(205.0,90.0,245.0), rgb(205.0,90.0,245.0), 0.0, 2); }
    }
}

const NB: i32 = 19;

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let D = length(u.canvas) * 0.5;
    let open = sat((u.since - 0.1) / 2.8);

    // tipo, famiglia (0 = lungo u ↗, 1 = lungo v ↘)
    var ty = array<i32, NB>(0, 1, 2, 3, 0, 9, 0, 2, 8, 4, 5, 6, 8, 0, 10, 7, 2, 10, 4);
    var fam = array<i32, NB>(0, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0);
    // u0, u1, v0, v1 in unità di mezza diagonale
    var bx = array<vec4f, NB>(
        vec4f(-1.3, 1.3, -1.15, -0.93), vec4f(0.62, 0.86, -1.3, -0.26), vec4f(-1.3, 0.95, -0.82, -0.62),
        vec4f(-1.3, -0.74, -0.98, -0.84), vec4f(0.24, 0.42, -1.3, -0.16), vec4f(0.18, 1.3, -0.63, -0.618),
        vec4f(-1.3, 0.62, -0.6, -0.45), vec4f(-0.18, 0.06, -1.3, -0.36), vec4f(0.06, 0.15, -0.78, -0.08),
        vec4f(0.55, 0.58, -1.3, -0.47), vec4f(-1.3, -0.12, -0.455, -0.425), vec4f(-1.3, -0.18, -0.425, -0.365),
        vec4f(-1.0, -0.05, -0.34, -0.27), vec4f(-1.3, 1.3, -0.26, -0.07), vec4f(-0.38, -0.372, -0.95, -0.16),
        vec4f(0.12, 0.92, -0.07, 0.025), vec4f(-0.6, 0.85, 0.025, 0.11), vec4f(-0.25, 1.05, 0.122, 0.13),
        vec4f(-1.3, 0.66, 0.155, 0.215));
    var neonIdx = array<i32, 9>(3, 5, 9, 10, 11, 14, 17, 18, 4);

    // sfondo
    let sd = sat(dot(p, u.canvas) / dot(u.canvas, u.canvas));
    var col = select(mix(rgb(9.0,10.0,20.0), rgb(5.0,5.0,8.0), (sd - 0.6) / 0.4), mix(rgb(11.0,15.0,28.0), rgb(9.0,10.0,20.0), sd / 0.6), sd < 0.6);
    let dz = length(p - vec2f(0.8 * W, 0.7 * H)) / (max(W, H) * 0.45);
    col = mix(col, rgb(60.0,40.0,110.0), (0.16 + 0.05 * sin(t * 0.4)) * sat(1.0 - dz));

    // lampo su una barra al neon
    var fb = -1;
    var fk = 0.0;
    if (t > 4.0 && u.since > 3.0) {
        let fc = floor((t - 4.0) / 6.5);
        fb = neonIdx[i32(floor(hash11(fc + 3.0) * 9.0))];
        fk = sin(min(1.0, ((t - 4.0) - fc * 6.5) / 2.0) * PI);
    }

    // riferimento ruotato di 45° con una leggera deriva
    let c0 = u.canvas * 0.5 + vec2f(sin(t * 0.05) * 8.0, cos(t * 0.04) * 6.0) * k;
    let q = p - c0;
    let Up = 0.70710678 * (q.x - q.y);
    let Vp = 0.70710678 * (q.x + q.y);

    for (var i = 0; i < NB; i++) {
        let fi = f32(i);
        let pl = pal(ty[i]);
        let spd = 0.08 + 0.12 * hash11(fi);
        let pha = 6.28 * hash11(fi + 50.0);
        let amp = (0.012 + 0.02 * f32(i % 3)) * D;
        let dirIn = select(-1.0, 1.0, i % 2 == 1);
        let intro = (1.0 - ease_out(open * 3.2 - fi * 0.07)) * dirIn * D * 1.6;
        let slide = sin(t * spd * 0.5 + pha) * amp + intro;
        var lx: f32;
        var ly: f32;
        var x0: f32;
        var x1: f32;
        var y0: f32;
        var y1: f32;
        let b = bx[i];
        if (fam[i] == 0) { lx = Up - slide; ly = Vp; x0 = b.x * D; x1 = b.y * D; y0 = b.z * D; y1 = b.w * D; }
        else { lx = Vp - slide; ly = Up; x0 = b.z * D; x1 = b.w * D; y0 = b.x * D; y1 = b.y * D; }
        let th = y1 - y0;
        let len = x1 - x0;
        let fl = select(0.0, fk, i == fb);

        // ombre morbide (verso il basso-destra e all'inizio della barra)
        if (pl.sh > 0.0) {
            if (lx > x0 && lx < x1 && ly > y1 && ly < y1 + 48.0 * k) { col *= 1.0 - pl.sh * (1.0 - (ly - y1) / (48.0 * k)); }
            if (lx > x0 - 30.0 * k && lx < x0 && ly > y0 && ly < y1 + 30.0 * k) { col *= 1.0 - pl.sh * 0.6 * (1.0 - (x0 - lx) / (30.0 * k)); }
        }
        if (lx < x0 || lx > x1) { continue; }

        if (pl.kind == 2) {
            // filo al neon
            let cy = (y0 + y1) * 0.5;
            let dy = abs(ly - cy);
            let R = 14.0 * k;
            let w = max(1.5 * k, th);
            if (dy < R) { col += pl.a * (1.0 - dy / R) * (0.35 + fl * 0.3); }
            col += mix(pl.a, vec3f(1.0), 0.25) * 0.95 * (1.0 - smoothstep(w * 0.5 - 0.5, w * 0.5 + 0.5, dy));
        } else if (ly >= y0 && ly <= y1) {
            let s = (lx - x0) / len;
            col = mix(mix(pl.a, vec3f(1.0), fl * 0.25), pl.b, (ly - y0) / th);
            // la barra si perde nel buio verso la fine
            col = mix(col, rgb(5.0, 5.0, 12.0), 0.55 * sat((s - 0.7) / 0.3));
            if (pl.kind == 3) {
                let hl = abs(fract(ly / (5.0 * k)) - 0.5) * 5.0 * k;
                col = mix(col, rgb(235.0,190.0,255.0), 0.22 * (1.0 - smoothstep(0.2 * k, 0.6 * k, hl)));
            }
            if (pl.kind == 4) {
                for (var n = 0; n < 9; n++) {
                    let fn_ = f32(n);
                    let yy = y0 + th * fn_ / 9.0;
                    if (abs(ly - yy) < 0.5 * k + 0.5 && lx > x0 + fn_ * 6.0 * k) {
                        col += rgb(112.0,189.0,232.0) * (0.08 + 0.18 * max(0.0, sin(t * 0.9 - fn_ * 0.6 + fi)));
                    }
                }
            }
            // filo di luce sul bordo superiore
            if (ly - y0 < max(1.0, k)) { col += mix(pl.a, vec3f(1.0), 0.35) * select(0.18, 0.55, pl.kind == 1); }
        }
        // riflesso che scorre sulle barre luminose
        if (pl.kind == 1 || pl.kind == 2) {
            let pos = (((t * (0.06 + spd * 0.4) + pha) % 1.6) - 0.3) * len + x0;
            let gl = 0.22 * len;
            let ext = select(0.0, 2.0 * k, pl.kind == 2);
            if (lx > pos - gl && lx < pos + 0.3 * gl && ly > y0 - ext && ly < y1 + ext) {
                let s = (lx - (pos - gl)) / (1.3 * gl);
                let prof = select((1.0 - s) / 0.2, s / 0.8, s < 0.8);
                col += vec3f(1.0) * (0.35 + fl * 0.3) * prof;
            }
        }
    }

    // zona scura dietro al logo e vignettatura
    let l = logo_layout();
    let dl = length(p - l.xy) / (l.z * 0.9 * 0.75);
    col = mix(col, rgb(4.0, 4.0, 10.0), 0.6 * sat(1.0 - dl) * u.logo_on);
    let dv = length(p - u.canvas * 0.5);
    let r0 = min(W, H) * 0.45;
    let r1 = length(u.canvas) * 0.62;
    col *= 1.0 - 0.45 * sat((dv - r0) / (r1 - r0));
    return col;
}
