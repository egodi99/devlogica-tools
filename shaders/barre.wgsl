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

// valori comuni a tutte le barre, calcolati una volta per pixel in scene()
var<private> gUp: f32;
var<private> gVp: f32;
var<private> gD: f32;
var<private> gK: f32;
var<private> gT: f32;
var<private> gOpen: f32;
var<private> gFb: i32;
var<private> gFk: f32;
// zero che il compilatore non può conoscere: impedisce che hash11() delle costanti venga
// calcolato in compilazione (con un seno diverso da quello del GPU, i riflessi si sposterebbero)
var<private> gZ: f32;

// Una barra: i = indice, ty = tipo, fam = famiglia (0 = lungo u ↗, 1 = lungo v ↘),
// b = u0, u1, v0, v1 in unità di mezza diagonale. Chiamata sempre con costanti,
// così il compilatore risolve pal() e i rami a tempo di compilazione.
fn bar(col_in: vec3f, i: i32, ty: i32, fam: i32, b: vec4f) -> vec3f {
    var col = col_in;
    let pl = pal(ty);
    let D = gD;
    let k = gK;
    let t = gT;
    var x0: f32;
    var x1: f32;
    var y0: f32;
    var y1: f32;
    var ly: f32;
    if (fam == 0) { ly = gVp; x0 = b.x * D; x1 = b.y * D; y0 = b.z * D; y1 = b.w * D; }
    else { ly = gUp; x0 = b.z * D; x1 = b.w * D; y0 = b.x * D; y1 = b.y * D; }
    let th = y1 - y0;

    // scarto rapido: la coordinata trasversale non dipende dallo scorrimento,
    // quindi si esclude la barra (ombre e aloni compresi) prima di ogni altro calcolo
    if (pl.kind == 2) {
        let m = max(14.0 * k, th * 0.5 + 2.0 * k + 1.0);
        if (abs(ly - (y0 + y1) * 0.5) > m) { return col; }
    } else {
        if (ly < y0 || ly > y1 + 48.0 * k) { return col; }
    }

    let fi = f32(i) + gZ;
    let spd = 0.08 + 0.12 * hash11(fi);
    let pha = 6.28 * hash11(fi + 50.0);
    let amp = (0.012 + 0.02 * f32(i % 3)) * D;
    let dirIn = select(-1.0, 1.0, i % 2 == 1);
    let intro = (1.0 - ease_out(gOpen * 3.2 - fi * 0.07)) * dirIn * D * 1.6;
    let slide = sin(t * spd * 0.5 + pha) * amp + intro;
    var lx: f32;
    if (fam == 0) { lx = gUp - slide; } else { lx = gVp - slide; }
    let len = x1 - x0;
    let fl = select(0.0, gFk, i == gFb);

    // ombre morbide (verso il basso-destra e all'inizio della barra)
    if (pl.sh > 0.0) {
        if (lx > x0 && lx < x1 && ly > y1 && ly < y1 + 48.0 * k) { col *= 1.0 - pl.sh * (1.0 - (ly - y1) / (48.0 * k)); }
        if (lx > x0 - 30.0 * k && lx < x0 && ly > y0 && ly < y1 + 30.0 * k) { col *= 1.0 - pl.sh * 0.6 * (1.0 - (x0 - lx) / (30.0 * k)); }
    }
    if (lx < x0 || lx > x1) { return col; }

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
            // nove linee equidistanti: basta guardare la più vicina
            let fn_ = clamp(round((ly - y0) / th * 9.0), 0.0, 8.0);
            let yy = y0 + th * fn_ / 9.0;
            if (abs(ly - yy) < 0.5 * k + 0.5 && lx > x0 + fn_ * 6.0 * k) {
                col += rgb(112.0,189.0,232.0) * (0.08 + 0.18 * max(0.0, sin(t * 0.9 - fn_ * 0.6 + fi)));
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
    return col;
}

// indice della barra al neon che riceve il lampo
fn neon_idx(j: i32) -> i32 {
    switch j {
        case 0: { return 3; }
        case 1: { return 5; }
        case 2: { return 9; }
        case 3: { return 10; }
        case 4: { return 11; }
        case 5: { return 14; }
        case 6: { return 17; }
        case 7: { return 18; }
        default: { return 4; }
    }
}

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let D = length(u.canvas) * 0.5;
    let open = sat((u.since - 0.1) / 2.8);

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
        fb = neon_idx(i32(floor(hash11(fc + 3.0) * 9.0)));
        fk = sin(min(1.0, ((t - 4.0) - fc * 6.5) / 2.0) * PI);
    }

    // riferimento ruotato di 45° con una leggera deriva
    let c0 = u.canvas * 0.5 + vec2f(sin(t * 0.05) * 8.0, cos(t * 0.04) * 6.0) * k;
    let q = p - c0;
    gUp = 0.70710678 * (q.x - q.y);
    gVp = 0.70710678 * (q.x + q.y);
    gD = D;
    gK = k;
    gT = t;
    gOpen = open;
    gFb = fb;
    gFk = fk;
    gZ = select(0.0, 1.0, u.time < -1e30);

    // barre, dal fondo verso l'alto: (indice, tipo, famiglia, u0 u1 v0 v1)
    col = bar(col,  0,  0, 0, vec4f(-1.3, 1.3, -1.15, -0.93));
    col = bar(col,  1,  1, 1, vec4f(0.62, 0.86, -1.3, -0.26));
    col = bar(col,  2,  2, 0, vec4f(-1.3, 0.95, -0.82, -0.62));
    col = bar(col,  3,  3, 0, vec4f(-1.3, -0.74, -0.98, -0.84));
    col = bar(col,  4,  0, 1, vec4f(0.24, 0.42, -1.3, -0.16));
    col = bar(col,  5,  9, 0, vec4f(0.18, 1.3, -0.63, -0.618));
    col = bar(col,  6,  0, 0, vec4f(-1.3, 0.62, -0.6, -0.45));
    col = bar(col,  7,  2, 1, vec4f(-0.18, 0.06, -1.3, -0.36));
    col = bar(col,  8,  8, 1, vec4f(0.06, 0.15, -0.78, -0.08));
    col = bar(col,  9,  4, 1, vec4f(0.55, 0.58, -1.3, -0.47));
    col = bar(col, 10,  5, 0, vec4f(-1.3, -0.12, -0.455, -0.425));
    col = bar(col, 11,  6, 0, vec4f(-1.3, -0.18, -0.425, -0.365));
    col = bar(col, 12,  8, 0, vec4f(-1.0, -0.05, -0.34, -0.27));
    col = bar(col, 13,  0, 0, vec4f(-1.3, 1.3, -0.26, -0.07));
    col = bar(col, 14, 10, 1, vec4f(-0.38, -0.372, -0.95, -0.16));
    col = bar(col, 15,  7, 0, vec4f(0.12, 0.92, -0.07, 0.025));
    col = bar(col, 16,  2, 0, vec4f(-0.6, 0.85, 0.025, 0.11));
    col = bar(col, 17, 10, 0, vec4f(-0.25, 1.05, 0.122, 0.13));
    col = bar(col, 18,  4, 0, vec4f(-1.3, 0.66, 0.155, 0.215));

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
