// nome: Onde di luce
// Esempio di sfondo personalizzato: copialo nella cartella "sfondi" (menu → Apri cartella sfondi
// personalizzati), poi menu → Ricarica sfondi. Puoi modificarlo liberamente.
//
// Hai a disposizione: u.canvas (dimensione tela), u.time (secondi), u.since (secondi dall'apertura),
// e le funzioni del preludio: rgb(), sat(), kpx(), stroke(), neon(), glow(), seg(), hash11()...

fn logo_layout() -> vec3f {
    // centro x, centro y, larghezza del logo (in pixel della tela)
    let w = min(0.26 * u.canvas.x, 0.46 * u.canvas.y) / 0.9;
    return vec3f(0.5 * u.canvas.x, 0.3 * u.canvas.y, w);
}

fn scene(p: vec2f) -> vec3f {
    let W = u.canvas.x;
    let H = u.canvas.y;
    let t = u.time;
    let x = p.x / W;
    var col = mix(rgb(2.0, 2.0, 10.0), rgb(8.0, 6.0, 30.0), sat(p.y / H));

    for (var i = 0; i < 7; i++) {
        let fi = f32(i);
        // ogni onda è una sinusoide lenta con fase e ampiezza diverse
        let y = H * (0.55 + 0.04 * fi)
              + sin(x * 3.0 + t * (0.15 + 0.03 * fi) + fi) * H * (0.06 + 0.01 * fi)
              + sin(x * 7.0 - t * 0.1 + fi * 2.0) * H * 0.015;
        let d = abs(p.y - y);
        let c = mix(rgb(60.0, 90.0, 255.0), rgb(170.0, 60.0, 240.0), fi / 6.0);
        // nastro morbido sotto la linea + filo luminoso
        let band = select(0.0, exp(-(p.y - y) / (H * 0.05)) * 0.18, p.y > y);
        col += c * band * (0.6 + 0.4 * sin(x * 4.0 + t * 0.3 + fi));
        col += mix(c, vec3f(1.0), 0.3) * neon(d, 1.2) * 0.5;
    }
    // vignettatura
    let dv = length(p - u.canvas * 0.5) / length(u.canvas * 0.5);
    return col * (1.0 - 0.5 * sat(dv - 0.4));
}
