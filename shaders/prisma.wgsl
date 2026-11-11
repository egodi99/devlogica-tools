// nome: Prisma
// tipo: fisso
// Cresta di sfaccettature di vetro a basso numero di poligoni, illuminate dall'alto, con spigoli al neon sottili.

fn logo_layout() -> vec3f {
    let w = min(0.26 * u.canvas.x, 0.46 * u.canvas.y) / 0.9;
    return vec3f(0.09 * u.canvas.x + w * 0.5, 0.5 * u.canvas.y, w);
}

// hash a due componenti per lo spostamento dei vertici
fn pr_h2(c: vec2f) -> vec2f {
    return vec2f(hash21(c), hash21(c + vec2f(17.31, 5.77))) - 0.5;
}

// "quota" della cresta: alta lungo una diagonale a ~50°, con onde lente e rumore per vertice
fn pr_height(v: vec2f, c: vec2f) -> f32 {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let q = (v - vec2f(0.69 * W, 0.62 * H)) / H;
    let n = normalize(vec2f(0.77, 0.64));         // normale alla cresta
    let across = dot(q, n);
    let along = dot(q, vec2f(-n.y, n.x));
    let ridge = exp(-across * across / 0.06) * (0.8 + 0.25 * sin(along * 5.0 + 1.3));
    return ridge * 1.1 + 0.2 * sin(q.x * 9.0 + 0.7) * cos(q.y * 7.0) + 0.45 * (hash21(c + 3.7) - 0.5);
}

// vertice (i, j) del reticolo triangolare, leggermente spostato
fn pr_vert(c: vec2f, a: vec2f, b: vec2f, jit: f32) -> vec2f {
    return c.x * a + c.y * b + pr_h2(c) * jit;
}

fn pr_edge(p: vec2f, a: vec2f, b: vec2f) -> f32 { return seg(p, a, b).x; }

fn pr_cross(a: vec2f, b: vec2f) -> f32 { return a.x * b.y - a.y * b.x; }

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();

    let SKY = rgb(112.0,189.0,232.0);
    let CY = rgb(86.0,214.0,246.0);
    let AZ = rgb(49.0,140.0,211.0);
    let IND = rgb(90.0,100.0,240.0);
    let VI = rgb(134.0,18.0,173.0);
    let LI = rgb(194.0,136.0,214.0);

    // ---- sfondo: inchiostro con una luce indaco in alto e una viola in basso a destra ----
    var col = rgb(5.0, 4.0, 14.0);
    let vm = max(W, H);
    col = mix(col, rgb(30.0, 34.0, 120.0), 0.42 * glow(length(p - vec2f(0.66 * W, 0.18 * H)), 0.55 * vm));
    col = mix(col, rgb(110.0, 22.0, 160.0), 0.30 * glow(length(p - vec2f(0.95 * W, 0.98 * H)), 0.42 * vm));

    // ---- reticolo triangolare con vertici spostati ----
    let S = 150.0 * min(W, H) / 1080.0;   // lato delle facce, proporzionato al lato corto
    let a = vec2f(S, 0.0);
    let b = vec2f(S * 0.5, S * 0.866);
    let det = pr_cross(a, b);
    let gi = pr_cross(p, b) / det;
    let gj = pr_cross(a, p) / det;
    let base = floor(vec2f(gi, gj));
    let jit = S * 0.34;

    var found = false;
    var A = vec2f(0.0);
    var B = vec2f(0.0);
    var C = vec2f(0.0);
    var cA = vec2f(0.0);
    var cB = vec2f(0.0);
    var cC = vec2f(0.0);
    for (var dj = -1; dj <= 1; dj++) {
        for (var di = -1; di <= 1; di++) {
            // salta le celle troppo lontane (lo spostamento dei vertici è al massimo ~0.3 celle)
            let fl = vec2f(gi, gj) - base - vec2f(f32(di), f32(dj));
            if (found || fl.x < -0.3 || fl.x > 1.3 || fl.y < -0.3 || fl.y > 1.3) { continue; }
            let c00 = base + vec2f(f32(di), f32(dj));
            let c10 = c00 + vec2f(1.0, 0.0);
            let c01 = c00 + vec2f(0.0, 1.0);
            let c11 = c00 + vec2f(1.0, 1.0);
            let v00 = pr_vert(c00, a, b, jit);
            let v10 = pr_vert(c10, a, b, jit);
            let v01 = pr_vert(c01, a, b, jit);
            let v11 = pr_vert(c11, a, b, jit);
            // due triangoli per cella
            for (var t = 0; t < 2; t++) {
                let pa = select(v10, v00, t == 0);
                let pb = select(v11, v10, t == 0);
                let pc = v01;
                let e0 = pr_cross(pb - pa, p - pa);
                let e1 = pr_cross(pc - pb, p - pb);
                let e2 = pr_cross(pa - pc, p - pc);
                if (!found && e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) {
                    found = true;
                    A = pa; B = pb; C = pc;
                    cA = select(c10, c00, t == 0);
                    cB = select(c11, c10, t == 0);
                    cC = c01;
                }
            }
        }
    }

    if (found) {
        let ctr = (A + B + C) / 3.0;
        // presenza della sfaccettatura: forte sulla cresta, sfuma verso il logo e verso l'alto a destra
        let n = normalize(vec2f(0.77, 0.64));
        let qc = (ctr - vec2f(0.69 * W, 0.62 * H)) / H;
        let across = dot(qc, n);
        let along = dot(qc, vec2f(-n.y, n.x));
        var pres = 0.18 + 0.82 * exp(-across * across / 0.05);
        pres *= sat(1.0 - abs(along) / 1.1);
        pres *= 0.7 + 0.3 * hash21(cA + cB + cC + 0.37);
        pres *= smoothstep(0.30 * W, 0.62 * W, ctr.x);

        // normale della faccia dalle quote dei tre vertici
        let z = 0.9 * H;
        let PA = vec3f(A, pr_height(A, cA) * z);
        let PB = vec3f(B, pr_height(B, cB) * z);
        let PC = vec3f(C, pr_height(C, cC) * z);
        var nrm = normalize(cross(PB - PA, PC - PA));
        if (nrm.z < 0.0) { nrm = -nrm; }
        let L = normalize(vec3f(-0.5, -0.8, 0.38));
        let lam = sat(dot(nrm, L));
        let spec = pow(sat(dot(nrm, normalize(L + vec3f(0.0, 0.0, 1.0)))), 24.0);

        // colore della faccia: azzurro in alto, indaco al centro, viola in basso
        let g = sat((ctr.y / H) * 1.1 - 0.05 + (hash21(cA * 1.3 + cC) - 0.5) * 0.25);
        var fc = select(mix(IND, VI, (g - 0.5) / 0.5), mix(AZ, IND, g / 0.5), g < 0.5);
        // qualche faccia "accesa" in ciano o lilla
        let hf = hash21(cA * 0.7 + cB * 1.9 + cC * 3.1);
        if (hf > 0.9) { fc = mix(fc, select(LI, CY, g < 0.5), 0.6); }
        // vetro: sfumatura diagonale dentro la faccia
        let dirL = normalize(vec2f(-0.45, -0.75));
        let r = max(max(length(A - ctr), length(B - ctr)), length(C - ctr));
        let s = sat(0.5 - dot(p - ctr, dirL) / (2.0 * r));
        let dark = rgb(6.0, 5.0, 20.0);
        let body = mix(dark, fc, 0.06 + 0.8 * lam * lam) * mix(1.15, 0.55, s);
        col = mix(col, body, 0.88 * pres);
        col += mix(fc, SKY, 0.5) * spec * 0.45 * pres;
        // facce accese: vetro colorato che brilla di luce propria
        if (hf > 0.9) { col += select(LI, CY, g < 0.5) * 0.10 * mix(1.0, 0.25, s) * pres; }

        // spigoli: sottili al neon, più accesi dove la faccia è più illuminata
        let dAB = pr_edge(p, A, B);
        let dBC = pr_edge(p, B, C);
        let dCA = pr_edge(p, C, A);
        let d = min(min(dAB, dBC), dCA);
        let ec = mix(mix(SKY, LI, g), vec3f(1.0), 0.15 * lam);
        col += ec * (0.08 + 0.4 * lam * lam) * pres * (stroke(d, 0.9) + 0.12 * stroke(d, 5.0));
        // alcuni spigoli (scelti per coppia di vertici, così coincidono tra facce vicine) al neon pieno
        var hot = 0.0;
        if (hash21(cA + cB + 0.5) > 0.86) { hot = max(hot, neon(dAB, 1.1)); }
        if (hash21(cB + cC + 0.5) > 0.86) { hot = max(hot, neon(dBC, 1.1)); }
        if (hash21(cC + cA + 0.5) > 0.86) { hot = max(hot, neon(dCA, 1.1)); }
        col += mix(mix(CY, LI, g), vec3f(1.0), 0.25) * hot * 0.8 * pres;
        // vertici: piccolo punto luminoso sui più alti
        let dv = min(min(length(p - A), length(p - B)), length(p - C));
        col += mix(CY, vec3f(1.0), 0.4) * 0.35 * pres * lam * glow(dv, 2.2 * k);
    }

    // ---- filo di luce lungo la cresta, inclinato come la barra "/" ----
    let n2 = normalize(vec2f(0.77, 0.64));
    let rc = vec2f(0.69 * W, 0.62 * H) - n2 * 0.30 * H;
    let lp = p - rc;
    let dl = abs(dot(lp, n2));
    let tl = dot(lp, vec2f(-n2.y, n2.x)) / H;
    let fl = sat(1.0 - abs(tl + 0.05) / 0.75) * smoothstep(0.40 * W, 0.7 * W, p.x);
    col += mix(CY, SKY, 0.5) * fl * (0.55 * stroke(dl, 1.0) + 0.06 * exp(-dl / (10.0 * k)));

    // lato sinistro più scuro, dove sta il logo
    let xr = p.x / (0.55 * W);
    let va = select(select(0.0, mix(0.3, 0.0, (xr - 0.6) / 0.4), xr < 1.0), mix(0.85, 0.3, xr / 0.6), xr < 0.6);
    col = mix(col, rgb(3.0, 2.0, 10.0), va);
    // vignettatura
    let dvg = length(p - u.canvas * 0.5);
    let r0 = min(W, H) * 0.4;
    let r1 = length(u.canvas) * 0.62;
    col *= 1.0 - 0.5 * sat((dvg - r0) / (r1 - r0));
    return col;
}
