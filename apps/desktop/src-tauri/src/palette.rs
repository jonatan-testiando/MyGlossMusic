//! Extraccion de paleta de la portada.
//!
//! # Por que Oklab y no RGB
//!
//! Agrupar colores en RGB da resultados sucios: la distancia euclidea en RGB no
//! se corresponde con la diferencia que percibe el ojo, asi que el k-means mezcla
//! colores que a la vista son distintos y separa otros que son casi iguales. El
//! resultado tipico es un marron indistinto.
//!
//! Oklab es perceptualmente uniforme: la distancia entre dos puntos si equivale
//! a lo diferentes que se ven. El mismo k-means, en este espacio, produce colores
//! que un humano reconoce como "los colores de esta portada".

use serde::Serialize;

/// Croma por debajo del cual un color es neutro a efectos practicos.
///
/// No es cero, y el margen importa. Medido sobre colores reales:
///
/// | color                        | croma |
/// |------------------------------|-------|
/// | blanco puro, gris medio      | 0,000 |
/// | blanco con matiz calido       | 0,019 |
/// | turquesa palido               | 0,023 |
/// | azul oscuro de verdad         | 0,033 |
/// | turquesa de verdad            | 0,051 |
/// | magenta                       | 0,169 |
///
/// El corte va en 0,028: por debajo son blancos y grises, por encima colores.
/// Empujar un blanco de 0,019 hasta 0,19 lo convertia en `#f0a36c`, un naranja
/// fuerte que NO esta en la portada, y la ventana salia rosa cuando la imagen
/// era turquesa.
const NEUTRAL: f32 = 0.028;

/// Cuanto peso conserva en la malla una zona sin color.
///
/// No es cero: un vestido blanco o un cielo palido son parte de la portada y
/// hacen falta para que la malla cubra la ventana. Pero no son "el color de
/// esta cancion", asi que no pueden mandar en la mezcla — y ocupan mucha
/// superficie, asi que por numero de pixeles mandarian.
const PESO_NEUTRO: f32 = 0.25;

/// Un color de la portada listo para pintar como luz ambiental.
///
/// La interfaz pinta un degradado radial por cada parada; el peso decide cual
/// ocupa mas superficie, para que la ventana se parezca a la portada y no a una
/// mezcla generica.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    pub color: String,
    /// Fraccion de la portada que ocupa este color, de 0 a 1.
    pub weight: f32,
}

/// Paleta lista para la interfaz.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    /// Colores de la malla ambiental, del que mas ocupa al que menos.
    ///
    /// Los cuatro campos de abajo son roles fijos derivados de estos; se
    /// conservan porque la interfaz los usa para texto, acentos y superficies,
    /// donde hace falta un color concreto y no una malla.
    pub stops: Vec<Stop>,
    /// Color dominante, oscurecido para servir de fondo.
    pub background: String,
    /// Segundo color del degradado.
    pub background_alt: String,
    /// Color de acento: el mas cromatico, para barras y resaltados.
    pub accent: String,
    /// Color de texto con contraste garantizado sobre `background`.
    pub foreground: String,
    /// `true` si el fondo es claro (la interfaz invierte los tonos).
    pub is_light: bool,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            stops: vec![
                Stop { color: "#3b3358".into(), weight: 0.5 },
                Stop { color: "#5b4a8a".into(), weight: 0.3 },
                Stop { color: "#2a2740".into(), weight: 0.2 },
            ],
            background: "#12101a".into(),
            background_alt: "#1c1826".into(),
            accent: "#8b7fd4".into(),
            foreground: "#f4f2fa".into(),
            is_light: false,
        }
    }
}

/// Un color en Oklab.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Oklab {
    l: f32,
    a: f32,
    b: f32,
}

impl Oklab {
    /// Croma: cuan saturado es el color. Sirve para elegir el acento.
    fn chroma(&self) -> f32 {
        (self.a * self.a + self.b * self.b).sqrt()
    }

    fn distance(&self, other: &Oklab) -> f32 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;
        dl * dl + da * da + db * db
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn rgb_to_oklab(r: u8, g: u8, b: u8) -> Oklab {
    let r = srgb_to_linear(r as f32 / 255.0);
    let g = srgb_to_linear(g as f32 / 255.0);
    let b = srgb_to_linear(b as f32 / 255.0);

    let l = (0.412_221_5 * r + 0.536_332_5 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();

    Oklab {
        l: 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        a: 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        b: 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    }
}

/// Componentes RGB lineales SIN recortar. Fuera de [0,1] el color no existe en
/// sRGB y no se puede pintar tal cual.
fn oklab_to_linear(c: Oklab) -> (f32, f32, f32) {
    let l = (c.l + 0.396_337_78 * c.a + 0.215_803_76 * c.b).powi(3);
    let m = (c.l - 0.105_561_346 * c.a - 0.063_854_17 * c.b).powi(3);
    let s = (c.l - 0.089_484_18 * c.a - 1.291_485_5 * c.b).powi(3);

    (
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    )
}

fn oklab_to_rgb(c: Oklab) -> (u8, u8, u8) {
    let (r, g, b) = oklab_to_linear(c);
    let f = |v: f32| (linear_to_srgb(v).clamp(0.0, 1.0) * 255.0).round() as u8;
    (f(r), f(g), f(b))
}

fn in_gamut(c: Oklab) -> bool {
    let (r, g, b) = oklab_to_linear(c);
    let ok = |v: f32| (-0.001..=1.001).contains(&v);
    ok(r) && ok(g) && ok(b)
}

/// Baja el croma hasta que el color cabe en sRGB, conservando tono y luminosidad.
///
/// Hace falta en cuanto se empuja el croma. La conversion recorta cada canal por
/// su cuenta, y eso NO conserva el tono: un morado fuera de gamut sale azul,
/// porque el canal rojo se recorta y el azul no. Bajar el croma pierde
/// intensidad, que es un fallo mucho menos visible que cambiar de color.
fn fit_gamut(c: Oklab) -> Oklab {
    if in_gamut(c) {
        return c;
    }
    let (mut cabe, mut no_cabe) = (0.0f32, 1.0f32);
    for _ in 0..14 {
        let t = (cabe + no_cabe) / 2.0;
        if in_gamut(Oklab { l: c.l, a: c.a * t, b: c.b * t }) {
            cabe = t;
        } else {
            no_cabe = t;
        }
    }
    Oklab { l: c.l, a: c.a * cabe, b: c.b * cabe }
}

/// Empuja el croma de un color hasta que se ve como color y no como gris.
///
/// # Por que hace falta empujar tanto
///
/// Un color vivo tiene croma 0,15-0,25 en Oklab. Pero los centros del k-means
/// no son colores de la imagen: son PROMEDIOS, y promediar pixeles cancela
/// color. Un grupo tipico sale con croma 0,02-0,08. Sin empujarlo, la ventana
/// queda gris por mucho que la portada sea vistosa.
///
/// Por debajo del umbral el color es gris de verdad y su tono es ruido de
/// redondeo: amplificarlo pintaria la ventana de un color inventado.
fn saturate(c: Oklab) -> Oklab {
    let chroma = c.chroma();
    if chroma < NEUTRAL {
        return c;
    }
    // El tope aviva un color apagado, pero no lo convierte en otro. Con 6x, un
    // gris con la mas leve desviacion salia como color saturado y la ventana se
    // teñia de algo que no estaba en la portada.
    let boost = (0.19 / chroma).clamp(0.9, 3.5);
    Oklab { l: c.l, a: c.a * boost, b: c.b * boost }
}

fn hex(c: Oklab) -> String {
    let (r, g, b) = oklab_to_rgb(c);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Extrae la paleta de los pixeles RGB de una portada ya reducida.
pub fn from_pixels(pixels: &[(u8, u8, u8)]) -> Palette {
    if pixels.is_empty() {
        return Palette::default();
    }

    let lab: Vec<Oklab> = pixels
        .iter()
        .map(|&(r, g, b)| rgb_to_oklab(r, g, b))
        .collect();

    let clusters = kmeans(&lab, 5, 12);
    if clusters.is_empty() {
        return Palette::default();
    }

    // Dominante: el grupo con mas pixeles.
    let dominant = clusters
        .iter()
        .max_by_key(|c| c.count)
        .map(|c| c.center)
        .unwrap_or(lab[0]);

    // Acento: el mas cromatico con peso suficiente para no ser un pixel perdido.
    let total: usize = clusters.iter().map(|c| c.count).sum();
    let accent = clusters
        .iter()
        .filter(|c| c.count * 20 > total) // al menos un 5% de la imagen
        .max_by(|a, b| {
            a.center
                .chroma()
                .partial_cmp(&b.center.chroma())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|c| c.center)
        .unwrap_or(dominant);

    // Paradas de la malla: los grupos con presencia real, del mas grande al mas
    // pequeno. Se descartan los que no llegan al 2% porque un pixel perdido de
    // color chillon tinaria toda la ventana.
    let mut ranked: Vec<&Cluster> = clusters.iter().filter(|c| c.count * 50 > total).collect();
    ranked.sort_by(|a, b| b.count.cmp(&a.count));

    // La malla pinta los COLORES de la portada, no su promedio. Las zonas sin
    // color se quedan — hacen falta para cubrir la ventana — pero con una
    // cuarta parte del peso, para que no manden solo por ocupar superficie.
    let peso = |c: &Cluster| {
        let base = c.count as f32;
        if c.center.chroma() >= NEUTRAL { base } else { base * PESO_NEUTRO }
    };

    // Los pesos se renormalizan sobre el total ya corregido: si no, restarle
    // peso a lo gris dejaria una malla que no llega a cubrir la ventana.
    let cubierto: f32 = ranked.iter().copied().map(peso).sum();
    let mut stops: Vec<Stop> = ranked
        .iter()
        .copied()
        .map(|c| Stop {
            color: hex(ambient(c.center)),
            weight: peso(c) / cubierto.max(f32::EPSILON),
        })
        .collect();
    stops.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));

    // El fondo y el acento salen del grupo con color de mas peso, no del que
    // mas pixeles tiene: ese puede ser justo la parte sin color.
    let cromatico_mayor = ranked
        .iter()
        .copied()
        .filter(|c| c.center.chroma() >= NEUTRAL)
        .max_by_key(|c| c.count)
        .map(|c| c.center);
    let dominant = cromatico_mayor.unwrap_or(dominant);
    let accent = cromatico_mayor
        .map(|_| {
            ranked
                .iter()
                .copied()
                .max_by(|a, b| {
                    a.center
                        .chroma()
                        .partial_cmp(&b.center.chroma())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|c| c.center)
                .unwrap_or(accent)
        })
        .unwrap_or(accent);

    build(stops, dominant, accent)
}

/// Lleva un color de la portada al rango en el que funciona como luz ambiental.
///
/// El limite de arriba evita que una portada clara apague el texto blanco; el de
/// abajo evita que una oscura se pierda contra el fondo y deje la ventana negra.
/// El croma se empuja hacia un valor fijo: sin esto, las portadas apagadas dan
/// una malla gris indistinguible del fondo por defecto.
fn ambient(c: Oklab) -> Oklab {
    fit_gamut(Oklab { l: c.l.clamp(0.52, 0.78), ..saturate(c) })
}

/// Ajusta luminosidad y contraste para que la paleta sea usable como interfaz.
///
/// Sin esto, una portada muy clara daria un fondo blanco con texto blanco.
fn build(stops: Vec<Stop>, dominant: Oklab, accent: Oklab) -> Palette {
    // El fondo se lleva a un rango oscuro fijo. Conserva el tono de la portada
    // (que es lo que da la sensacion de "la app se tinta con la cancion") pero
    // garantiza que el texto claro siempre contraste.
    // Los cuatro roles parten del croma ya empujado: si no, el acento sale
    // apagado y el fondo casi neutro, por el mismo motivo que las paradas.
    let dom = saturate(dominant);
    let acc = saturate(accent);

    let bg = fit_gamut(Oklab { l: 0.18, a: dom.a * 0.45, b: dom.b * 0.45 });
    let bg_alt = fit_gamut(Oklab { l: 0.26, a: acc.a * 0.4, b: acc.b * 0.4 });
    let ac = fit_gamut(Oklab { l: 0.72, a: acc.a, b: acc.b });
    // El texto solo se tinta un poco: mas y deja de leerse como blanco.
    let fg = fit_gamut(Oklab { l: 0.97, a: dominant.a * 0.08, b: dominant.b * 0.08 });

    Palette {
        stops,
        background: hex(bg),
        background_alt: hex(bg_alt),
        accent: hex(ac),
        foreground: hex(fg),
        is_light: false,
    }
}

struct Cluster {
    center: Oklab,
    count: usize,
}

/// K-means sobre Oklab. `k` grupos, `iters` iteraciones.
///
/// Inicializacion determinista (muestreo uniforme del conjunto) para que la
/// misma portada de siempre la misma paleta: un fondo que cambia de color entre
/// ejecuciones se percibe como un fallo.
fn kmeans(points: &[Oklab], k: usize, iters: usize) -> Vec<Cluster> {
    if points.is_empty() {
        return Vec::new();
    }
    let k = k.min(points.len());

    let step = points.len() / k;
    let mut centers: Vec<Oklab> = (0..k).map(|i| points[i * step]).collect();

    let mut assign = vec![0usize; points.len()];

    for _ in 0..iters {
        let mut moved = false;
        for (i, p) in points.iter().enumerate() {
            let best = centers
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    p.distance(a)
                        .partial_cmp(&p.distance(b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            if assign[i] != best {
                assign[i] = best;
                moved = true;
            }
        }

        let mut sums = vec![(0.0f32, 0.0f32, 0.0f32, 0usize); k];
        for (i, p) in points.iter().enumerate() {
            let s = &mut sums[assign[i]];
            s.0 += p.l;
            s.1 += p.a;
            s.2 += p.b;
            s.3 += 1;
        }
        for (c, s) in centers.iter_mut().zip(&sums) {
            if s.3 > 0 {
                let n = s.3 as f32;
                *c = Oklab {
                    l: s.0 / n,
                    a: s.1 / n,
                    b: s.2 / n,
                };
            }
        }

        if !moved {
            break; // convergio
        }
    }

    let mut counts = vec![0usize; k];
    for &a in &assign {
        counts[a] += 1;
    }
    centers
        .into_iter()
        .zip(counts)
        .filter(|(_, count)| *count > 0)
        .map(|(center, count)| Cluster { center, count })
        .collect()
}

/// Descarga una portada y extrae su paleta.
pub async fn from_url(http: &reqwest::Client, url: &str) -> anyhow::Result<Palette> {
    let bytes = http.get(url).send().await?.error_for_status()?.bytes().await?;
    from_bytes(&bytes)
}

/// Extrae la paleta de una imagen ya descargada.
pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Palette> {
    let img = image::load_from_memory(bytes)?;
    // 48x48 basta de sobra: la paleta es una estadistica, no un detalle. Reducir
    // primero hace el k-means ~100 veces mas barato.
    let small = img.resize_exact(48, 48, image::imageops::FilterType::Triangle);
    let rgb = small.to_rgb8();

    let pixels: Vec<(u8, u8, u8)> = rgb.pixels().map(|p| (p[0], p[1], p[2])).collect();
    Ok(from_pixels(&pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta_de_color() {
        for c in [(255u8, 0u8, 0u8), (0, 128, 255), (30, 30, 30), (255, 255, 255)] {
            let (r, g, b) = oklab_to_rgb(rgb_to_oklab(c.0, c.1, c.2));
            assert!(
                r.abs_diff(c.0) <= 2 && g.abs_diff(c.1) <= 2 && b.abs_diff(c.2) <= 2,
                "{c:?} -> {:?}",
                (r, g, b)
            );
        }
    }

    #[test]
    fn una_portada_azul_da_paleta_azul() {
        let pixels: Vec<_> = (0..500).map(|i| (20, 40 + (i % 20) as u8, 180)).collect();
        let p = from_pixels(&pixels);
        let (r, _g, b) = (
            u8::from_str_radix(&p.accent[1..3], 16).unwrap(),
            0,
            u8::from_str_radix(&p.accent[5..7], 16).unwrap(),
        );
        assert!(b > r, "el acento deberia ser azulado, salio {}", p.accent);
    }

    #[test]
    fn el_fondo_siempre_es_oscuro_aunque_la_portada_sea_blanca() {
        let pixels: Vec<_> = (0..200).map(|_| (255u8, 255u8, 255u8)).collect();
        let p = from_pixels(&pixels);
        let bg = u32::from_str_radix(&p.background[1..], 16).unwrap();
        let (r, g, b) = ((bg >> 16) & 255, (bg >> 8) & 255, bg & 255);
        let lum = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
        assert!(lum < 90.0, "fondo demasiado claro: {} (lum {lum})", p.background);
    }

    #[test]
    fn siempre_hay_paradas_para_la_malla() {
        // Una portada de un solo color sigue teniendo que dar algo que pintar:
        // si `stops` viniera vacio, la ventana se quedaria negra.
        let pixels: Vec<_> = (0..300).map(|_| (18u8, 40u8, 190u8)).collect();
        let p = from_pixels(&pixels);
        assert!(!p.stops.is_empty());
        let suma: f32 = p.stops.iter().map(|s| s.weight).sum();
        assert!(suma > 0.9 && suma <= 1.001, "los pesos no cubren la portada: {suma}");
    }

    #[test]
    fn las_paradas_van_de_mayor_a_menor_peso() {
        let pixels: Vec<_> = (0..600)
            .map(|i| if i % 6 == 0 { (250u8, 30u8, 40u8) } else { (20u8, 30u8, 120u8) })
            .collect();
        let p = from_pixels(&pixels);
        for par in p.stops.windows(2) {
            assert!(par[0].weight >= par[1].weight, "paradas desordenadas: {:?}", p.stops);
        }
    }

    #[test]
    fn una_portada_apagada_sigue_dando_color() {
        // Un turquesa apagado como el de una portada real (croma ~0,05): tiene
        // color de verdad, solo poco. Es lo que hay que avivar.
        //
        // Este test pedia antes lo mismo de un gris pizarra (croma 0,02), y esa
        // expectativa ERA el fallo: cumplirla obligaba a un empuje que
        // convertia los blancos calidos en naranjas inventados.
        let pixels: Vec<_> = (0..400)
            .map(|i| (58u8, 112u8 + (i % 6) as u8, 128u8))
            .collect();
        let p = from_pixels(&pixels);

        for parada in &p.stops {
            assert!(
                amplitud(&parada.color) > 45,
                "un turquesa de verdad salio apagado: {}",
                parada.color
            );
        }
    }

    #[test]
    fn un_gris_de_verdad_no_se_inventa_color() {
        // Al reves del anterior: si la portada es gris, su tono es ruido de
        // redondeo. Amplificarlo pintaria la ventana de un color que no existe.
        let pixels: Vec<_> = (0..200).map(|_| (128u8, 128u8, 128u8)).collect();
        let p = from_pixels(&pixels);

        for parada in &p.stops {
            let (r, g, b) = (
                u8::from_str_radix(&parada.color[1..3], 16).unwrap() as i32,
                u8::from_str_radix(&parada.color[3..5], 16).unwrap() as i32,
                u8::from_str_radix(&parada.color[5..7], 16).unwrap() as i32,
            );
            let amplitud = [r, g, b].iter().max().unwrap() - [r, g, b].iter().min().unwrap();
            assert!(amplitud < 12, "gris teñido: {}", parada.color);
        }
    }

    #[test]
    fn ajustar_al_gamut_conserva_el_tono() {
        // Un morado imposible de pintar. Recortando canales saldria azul; el
        // ajuste tiene que dejarlo morado y solo bajarle intensidad.
        let imposible = Oklab { l: 0.6, a: 0.42, b: -0.36 };
        assert!(!in_gamut(imposible));

        let ajustado = fit_gamut(imposible);
        assert!(in_gamut(ajustado));
        assert!(ajustado.chroma() < imposible.chroma());

        let tono = |c: &Oklab| c.b.atan2(c.a);
        assert!(
            (tono(&ajustado) - tono(&imposible)).abs() < 0.02,
            "el tono se movio: {} -> {}",
            tono(&imposible),
            tono(&ajustado)
        );
    }

    /// Amplitud entre canales: sirve de medida barata de "cuanto color tiene".
    fn amplitud(hex: &str) -> i32 {
        let c: Vec<i32> = (1..7)
            .step_by(2)
            .map(|i| i32::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        c.iter().max().unwrap() - c.iter().min().unwrap()
    }

    #[test]
    fn las_barras_negras_no_tinen_la_ventana_de_gris() {
        // Caso real: `hqdefault.jpg` es 4:3 con barras negras incrustadas, y en
        // la portada medida el negro era el 23% de los pixeles — el grupo mas
        // grande. Sin filtrar, el foco principal salia gris (#686969) y tapaba
        // el turquesa que si estaba en la imagen.
        let mut pixels: Vec<(u8, u8, u8)> = (0..500).map(|_| (0u8, 0u8, 0u8)).collect();
        pixels.extend((0..900).map(|i| (60u8, 120u8 + (i % 20) as u8, 135u8)));
        pixels.extend((0..600).map(|i| (40u8, 70u8, 90u8 + (i % 15) as u8)));

        let p = from_pixels(&pixels);
        let principal = &p.stops[0];
        assert!(
            amplitud(&principal.color) > 25,
            "el foco principal salio gris: {}",
            principal.color
        );

        // Y el gris, en conjunto, no puede llevarse la mayoria de la mezcla.
        let peso_gris: f32 = p
            .stops
            .iter()
            .filter(|s| amplitud(&s.color) <= 25)
            .map(|s| s.weight)
            .sum();
        assert!(peso_gris < 0.35, "el gris manda en la malla: {peso_gris}");
    }

    #[test]
    fn un_blanco_calido_no_se_convierte_en_naranja() {
        // El otro caso real: el vestido y las nubes son blanco con matiz calido
        // (croma 0,019). Empujandolo salia #f0a36c, un naranja que no existe en
        // la portada. Tiene que quedarse cerca del blanco.
        let mut pixels: Vec<(u8, u8, u8)> = (0..900).map(|i| (247u8, 234u8, 225u8 - (i % 6) as u8)).collect();
        // Un turquesa minoritario, para que haya dos grupos y el filtro actue.
        pixels.extend((0..300).map(|_| (60u8, 130u8, 140u8)));

        let p = from_pixels(&pixels);
        let blanco = p
            .stops
            .iter()
            .find(|s| {
                let r = i32::from_str_radix(&s.color[1..3], 16).unwrap();
                r > 180
            })
            .expect("el blanco calido deberia seguir estando en la malla");
        assert!(
            amplitud(&blanco.color) < 45,
            "el blanco se volvio color: {}",
            blanco.color
        );
    }

    #[test]
    fn los_pesos_cubren_la_ventana_tras_descartar_lo_gris() {
        // Descartar el 40% de la portada sin renormalizar dejaria una malla que
        // no llega a cubrir la ventana.
        let mut pixels: Vec<(u8, u8, u8)> = (0..600).map(|_| (250u8, 250u8, 250u8)).collect();
        pixels.extend((0..500).map(|i| (30u8, 110u8 + (i % 10) as u8, 160u8)));
        pixels.extend((0..400).map(|i| (170u8, 40u8, 90u8 + (i % 12) as u8)));

        let p = from_pixels(&pixels);
        let suma: f32 = p.stops.iter().map(|s| s.weight).sum();
        assert!(
            (suma - 1.0).abs() < 0.02,
            "los pesos no cubren la ventana: {suma}"
        );
    }

    #[test]
    fn es_determinista() {
        let pixels: Vec<_> = (0..300).map(|i| ((i % 255) as u8, 60, 120)).collect();
        assert_eq!(from_pixels(&pixels).accent, from_pixels(&pixels).accent);
    }
}
