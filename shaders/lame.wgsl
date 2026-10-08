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

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let k = kpx();
    let t = u.time;
    let open = ease_out((u.since - 0.2) / 2.6);

    var a0 = array<f32, 6>(-31.0, -25.5, -20.0, -14.5, -9.5, -5.0);
    var a1 = array<f32, 6>(-38.5, -31.6, -26.0, -20.4, -14.9, -9.9);
    var edge = array<vec3f, 6>(rgb(112.0,189.0,232.0), rgb(150.0,170.0,255.0), rgb(194.0,136.0,214.0), rgb(214.0,150.0,240.0), rgb(194.0,136.0,214.0), rgb(170.0,120.0,230.0));
    var body = array<vec3f, 6>(rgb(49.0,140.0,211.0), rgb(52.0,60.0,200.0), rgb(96.0,58.0,214.0), rgb(134.0,18.0,173.0), rgb(104.0,24.0,160.0), rgb(85.0,0.0,124.0));
    var sheen = array<f32, 6>(0.55, 0.75, 0.9, 0.7, 0.5, 0.4);
    var ph = array<f32, 6>(0.0, 1.3, 2.1, 3.4, 4.2, 5.0);

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
    let ang = degrees(atan2(v.y, v.x));

    // lampo periodico su una lama a caso
    var fb = -1;
    var flareK = 0.0;
    if (t > 4.0 && u.since > 3.0) {
        let fc = floor((t - 4.0) / 7.5);
        fb = i32(floor(hash11(fc) * 6.0));
        flareK = sin(min(1.0, ((t - 4.0) - fc * 7.5) / 2.2) * PI);
    }

    var lo: array<f32, 6>;
    var hi: array<f32, 6>;
    var fade: array<f32, 6>;
    var top = -1;
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let breath = 2.2 * sin(t * 0.22) * (fi / 5.0 - 0.5);
        let ripple = 1.6 * sin(t * 0.55 - fi * 0.75);
        let wob = 2.2 * sin(t * 0.21 + ph[i]) + 0.9 * sin(t * 0.083 + ph[i] * 2.0) + breath;
        lo[i] = a0[i] + wob;
        hi[i] = mix(a0[i], a1[i], open) + wob * 0.9 + ripple * open;
        fade[i] = sat(open * 1.4 - fi * 0.08);
        if (fade[i] > 0.0 && ang >= hi[i] && ang <= lo[i]) { top = i; }
    }

    // ---- corpo della lama più in alto in questo pixel ----
    if (top >= 0) {
        let i = top;
        let fl = select(0.0, flareK, i == fb);
        let f = fade[i];
        let mid = (lo[i] + hi[i]) * 0.5;
        let tt = r * cos(radians(ang - mid)) / L;
        let dark = rgb(6.0, 5.0, 20.0);
        var c: vec3f;
        if (tt < 0.25) { c = mix(dark, body[i], 0.22 + fl * 0.2); }
        else if (tt < 0.6) { c = mix(mix(dark, body[i], 0.22 + fl * 0.2), mix(dark, body[i], 0.55 + fl * 0.3), (tt - 0.25) / 0.35); }
        else { c = mix(mix(dark, body[i], 0.55 + fl * 0.3), mix(dark, body[i], 0.12), sat((tt - 0.6) / 0.4)); }
        let a = ramp4(tt, 0.0, 0.92 * f, 0.95 * f, 0.95 * f, 0.25, 0.6);
        col = mix(col, c, a);
        // ombra lungo il bordo inferiore: stacca i piani
        let dlo = r * sin(radians(lo[i] - ang));
        col = mix(col, rgb(2.0, 1.0, 8.0), 0.55 * f * sat(1.0 - dlo / (90.0 * k)));
        // lucentezza sulla parte alta
        let sheenA = hi[i] + (lo[i] - hi[i]) * 0.42;
        if (ang < sheenA) {
            for (var s = 0; s < 2; s++) {
                let spd = select(0.07, 0.11, s == 0);
                let off = select(0.8, 0.0, s == 0);
                let amp = select(0.22, 0.36, s == 0);
                let sw = ((t * spd + ph[i] * 0.17 + off) % 1.7) - 0.35;
                var prof = 0.0;
                if (tt > sw - 0.22 && tt <= sw) { prof = (tt - sw + 0.22) / 0.22; }
                else if (tt > sw && tt < sw + 0.26) { prof = 1.0 - (tt - sw) / 0.26; }
                col += edge[i] * amp * sheen[i] * f * prof;
            }
            var add: vec3f;
            if (tt < 0.45) { add = body[i] * (0.10 + fl * 0.25) * sat(tt / 0.45); }
            else if (tt < 0.7) { add = mix(body[i] * (0.10 + fl * 0.25), edge[i] * (0.22 + fl * 0.35), (tt - 0.45) / 0.25); }
            else { add = mix(edge[i] * (0.22 + fl * 0.35), body[i] * 0.04, sat((tt - 0.7) / 0.3)); }
            col += add * sheen[i] * f;
        }
    }

    // ---- bordi al neon con impulsi che corrono ----
    for (var i = 0; i < 6; i++) {
        if (fade[i] <= 0.0) { continue; }
        let dA = radians(ang - hi[i]);
        let along = r * cos(dA);
        if (along <= 0.0) { continue; }
        let d = abs(r * sin(dA));
        if (d > 60.0 * k) { continue; }
        let tt = along / L;
        let fl = select(0.0, flareK, i == fb);
        var inten = select(mix(0.25 + fl * 0.4, 0.2 + fl * 0.3, (tt - 0.16) / 0.84), (0.25 + fl * 0.4) * tt / 0.16, tt < 0.16);
        var hot = 0.0;
        for (var s = 0; s < 2; s++) {
            let spd = select(0.11, 0.17, s == 0);
            let off = select(0.7, 0.0, s == 0);
            let pos = ((t * spd + ph[i] * 0.23 + off) % 1.5) - 0.25;
            if (pos > 0.2 && pos < 0.98) {
                let q = abs(tt - pos);
                hot = max(hot, sat(1.0 - q / 0.1));
                inten += 0.18 * exp(-q * q / 0.02);
            }
        }
        inten = (inten + hot * 0.6) * fade[i];
        let c = mix(edge[i], vec3f(1.0), 0.65 * hot);
        let prof = stroke(d, 12.0) * (0.08 + fl * 0.08) + stroke(d, 4.0) * 0.2 + stroke(d, 1.3);
        col += c * inten * prof;
    }

    // ---- polvere luminosa che sale lungo le lame ----
    if (open > 0.6) {
        for (var b = 0; b < 6; b++) {
            for (var n = 0; n < 10; n++) {
                let seed = f32(b * 10 + n);
                let pd = 4.0 + 7.0 * hash11(seed);
                let tt0 = t + hash11(seed + 0.5) * pd;
                let cyc = floor(tt0 / pd);
                let lt = tt0 - cyc * pd;
                let sd = seed * 13.0 + cyc * 7.1;
                let rr = 0.12 + 0.1 * hash11(sd + 1.0) + (0.025 + 0.05 * hash11(sd + 2.0)) * lt;
                let an = radians(mix(lo[b], hi[b], hash11(sd)));
                let q = P + vec2f(cos(an), sin(an)) * L * rr;
                let rad = (0.6 + 1.6 * hash11(sd + 3.0)) * 2.6 * k;
                let d = length(p - q);
                if (d < rad) {
                    let fd = min(1.0, lt / 1.5) * sat((pd - lt) / 1.5) * (0.6 + 0.4 * sin(t * 2.0 + 6.28 * hash11(sd + 4.0)));
                    let x = d / rad;
                    let c = select(mix(edge[b] * 0.35, vec3f(0.0), (x - 0.4) / 0.6), mix(vec3f(0.7), edge[b] * 0.35, x / 0.4), x < 0.4);
                    col += c * fd;
                }
            }
        }
    }

    // ---- filamenti sottili che scivolano lungo il ventaglio ----
    var fa = array<f32, 5>(-36.5, -28.4, -17.3, -11.6, -6.2);
    var fal = array<f32, 5>(0.3, 0.2, 0.22, 0.16, 0.16);
    var fsp = array<f32, 5>(0.13, 0.09, 0.11, 0.07, 0.1);
    var fcol = array<vec3f, 5>(rgb(112.0,189.0,232.0), rgb(150.0,170.0,255.0), rgb(194.0,136.0,214.0), rgb(214.0,150.0,240.0), rgb(150.0,120.0,230.0));
    for (var j = 0; j < 5; j++) {
        let fj = f32(j);
        let an = radians(fa[j] + 2.0 * sin(t * 0.2 + fj * 1.7));
        let dir = vec2f(cos(an), sin(an));
        let nrm = vec2f(-dir.y, dir.x) * 14.0 * k;
        let s = ((t * fsp[j] + fj * 0.37) % 1.3) - 0.15;
        let sg = seg(p, P + dir * L * s + nrm, P + dir * L * (s + 0.3) + nrm);
        if (sg.x < 3.0 * k) {
            let g = select(mix(1.0, 0.0, (sg.y - 0.6) / 0.4), sg.y / 0.6, sg.y < 0.6);
            col += fcol[j] * fal[j] * open * g * stroke(sg.x, 1.2);
        }
    }

    // bagliore pulsante all'origine del ventaglio
    let pr = min(W, H) * (0.32 + 0.05 * sin(t * 0.8));
    col += rgb(120.0, 80.0, 255.0) * 0.15 * open * sat(1.0 - length(p - P) / pr);

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
