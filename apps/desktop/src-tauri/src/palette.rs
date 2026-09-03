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

fn oklab_to_rgb(c: Oklab) -> (u8, u8, u8) {
    let l = (c.l + 0.396_337_78 * c.a + 0.215_803_76 * c.b).powi(3);
    let m = (c.l - 0.105_561_346 * c.a - 0.063_854_17 * c.b).powi(3);
    let s = (c.l - 0.089_484_18 * c.a - 1.291_485_5 * c.b).powi(3);

    let r = 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s;
    let g = -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s;
    let b = -0.004_196_086 * l - 0.703_418_6 * m + 1.707_614_7 * s;

    let f = |v: f32| (linear_to_srgb(v).clamp(0.0, 1.0) * 255.0).round() as u8;
    (f(r), f(g), f(b))
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
    let stops: Vec<Stop> = ranked
        .iter()
        .map(|c| Stop {
            color: hex(ambient(c.center)),
            weight: c.count as f32 / total as f32,
        })
        .collect();

    build(stops, dominant, accent)
}

/// Lleva un color de la portada al rango en el que funciona como luz ambiental.
///
/// El limite de arriba evita que una portada clara apague el texto blanco; el de
/// abajo evita que una oscura se pierda contra el fondo y deje la ventana negra.
/// El croma se empuja hacia un valor fijo: sin esto, las portadas apagadas dan
/// una malla gris indistinguible del fondo por defecto.
fn ambient(c: Oklab) -> Oklab {
    let boost = (0.16 / c.chroma().max(0.01)).clamp(0.85, 2.2);
    Oklab {
        l: c.l.clamp(0.42, 0.70),
        a: c.a * boost,
        b: c.b * boost,
    }
}

/// Ajusta luminosidad y contraste para que la paleta sea usable como interfaz.
///
/// Sin esto, una portada muy clara daria un fondo blanco con texto blanco.
fn build(stops: Vec<Stop>, dominant: Oklab, accent: Oklab) -> Palette {
    // El fondo se lleva a un rango oscuro fijo. Conserva el tono de la portada
    // (que es lo que da la sensacion de "la app se tinta con la cancion") pero
    // garantiza que el texto claro siempre contraste.
    let bg = Oklab {
        l: 0.16,
        a: dominant.a * 0.55,
        b: dominant.b * 0.55,
    };
    let bg_alt = Oklab {
        l: 0.24,
        a: accent.a * 0.45,
        b: accent.b * 0.45,
    };
    // El acento se satura y se sube de luminosidad para que destaque.
    let chroma = accent.chroma().max(0.02);
    let boost = (0.16 / chroma).min(2.2);
    let ac = Oklab {
        l: 0.72,
        a: accent.a * boost,
        b: accent.b * boost,
    };
    let fg = Oklab {
        l: 0.97,
        a: dominant.a * 0.08,
        b: dominant.b * 0.08,
    };

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
    fn es_determinista() {
        let pixels: Vec<_> = (0..300).map(|i| ((i % 255) as u8, 60, 120)).collect();
        assert_eq!(from_pixels(&pixels).accent, from_pixels(&pixels).accent);
    }
}
