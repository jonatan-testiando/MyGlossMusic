//! Cola de reproduccion.

use serde::{Deserialize, Serialize};
use ytm_source::TrackInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    /// Al acabar la cola, para.
    Off,
    /// Al acabar la cola, vuelve al principio.
    All,
    /// Repite la pista actual indefinidamente.
    One,
}

#[derive(Debug, Default)]
pub struct Queue {
    items: Vec<TrackInfo>,
    /// Indice en `items` de la pista actual.
    index: usize,
    /// Orden barajado: posiciones dentro de `items`. Vacio si no hay shuffle.
    order: Vec<usize>,
    repeat: Repeat,
    shuffle: bool,
}

impl Default for Repeat {
    fn default() -> Self {
        Repeat::Off
    }
}

impl Queue {
    pub fn set_items(&mut self, items: Vec<TrackInfo>, start: usize) {
        self.items = items;
        self.index = start.min(self.items.len().saturating_sub(1));
        if self.shuffle {
            self.reshuffle();
        }
    }

    /// Sustituye lo que viene DESPUES de la pista actual, sin tocarla.
    ///
    /// Es lo que necesita la radio: la cancion ya esta sonando cuando llegan
    /// las recomendaciones, y volver a poner la cola entera la reiniciaria
    /// desde cero. Devuelve cuantas pistas quedaron detras.
    pub fn set_up_next(&mut self, items: Vec<TrackInfo>) -> usize {
        self.items.truncate(self.index + 1);
        self.items.extend(items);
        if self.shuffle {
            self.reshuffle();
        }
        self.items.len().saturating_sub(self.index + 1)
    }

    /// Mete una pista justo despues de la que suena.
    ///
    /// A diferencia de [`Self::set_up_next`], que sustituye toda la cola de
    /// detras, esta la conserva: es "reproducir a continuacion", no "cambiar de
    /// cola". Si la pista ya estaba en la cola por detras, se mueve en vez de
    /// duplicarse — tener la misma cancion dos veces seguidas nunca es lo que
    /// se pretendia.
    pub fn play_next(&mut self, item: TrackInfo) {
        if let Some(pos) = self
            .items
            .iter()
            .position(|t| t.video_id == item.video_id && t.video_id != self.current_id())
        {
            self.items.remove(pos);
            // Quitar algo de delante corre el indice de la actual hacia atras.
            if pos < self.index {
                self.index -= 1;
            }
        }
        let destino = (self.index + 1).min(self.items.len());
        self.items.insert(destino, item);
    }

    fn current_id(&self) -> &str {
        self.items.get(self.index).map_or("", |t| t.video_id.as_str())
    }

    pub fn items(&self) -> &[TrackInfo] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn current(&self) -> Option<&TrackInfo> {
        self.items.get(self.index)
    }

    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    pub fn set_repeat(&mut self, r: Repeat) {
        self.repeat = r;
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.shuffle = on;
        if on {
            self.reshuffle();
        } else {
            self.order.clear();
        }
    }

    /// Anade al final y devuelve su posicion.
    pub fn push(&mut self, track: TrackInfo) -> usize {
        self.items.push(track);
        if self.shuffle {
            self.order.push(self.items.len() - 1);
        }
        self.items.len() - 1
    }

    /// Reemplaza los metadatos de la pista actual.
    ///
    /// La cola puede construirse con ids pelados (p.ej. desde `PlayNow`); el
    /// titulo y el autor reales solo se conocen al resolver la pista.
    pub fn set_current_meta(&mut self, meta: TrackInfo) {
        if let Some(slot) = self.items.get_mut(self.index) {
            *slot = meta;
        }
    }

    /// Salta a una posicion concreta.
    pub fn jump_to(&mut self, index: usize) -> Option<&TrackInfo> {
        if index < self.items.len() {
            self.index = index;
            self.current()
        } else {
            None
        }
    }

    /// Avanza. `auto` distingue el fin natural de una pista (donde `Repeat::One`
    /// repite) de un salto pedido por el usuario (donde siempre avanza).
    pub fn next(&mut self, auto: bool) -> Option<&TrackInfo> {
        if self.items.is_empty() {
            return None;
        }
        if auto && self.repeat == Repeat::One {
            return self.current();
        }

        let pos = self.position_in_order();
        let last = self.items.len() - 1;

        if pos >= last {
            match self.repeat {
                Repeat::Off => return None,
                _ => self.index = self.nth_in_order(0),
            }
        } else {
            self.index = self.nth_in_order(pos + 1);
        }
        self.current()
    }

    /// Retrocede. Al principio de la cola se queda donde esta.
    pub fn prev(&mut self) -> Option<&TrackInfo> {
        if self.items.is_empty() {
            return None;
        }
        let pos = self.position_in_order();
        if pos == 0 {
            if self.repeat == Repeat::All {
                self.index = self.nth_in_order(self.items.len() - 1);
            }
        } else {
            self.index = self.nth_in_order(pos - 1);
        }
        self.current()
    }

    /// La pista que sonara despues, para precargarla sin alterar el estado.
    pub fn peek_next(&self) -> Option<&TrackInfo> {
        if self.items.is_empty() {
            return None;
        }
        if self.repeat == Repeat::One {
            return self.current();
        }
        let pos = self.position_in_order();
        if pos + 1 < self.items.len() {
            self.items.get(self.nth_in_order(pos + 1))
        } else if self.repeat == Repeat::All {
            self.items.get(self.nth_in_order(0))
        } else {
            None
        }
    }

    /// Posicion de la pista actual dentro del orden efectivo.
    fn position_in_order(&self) -> usize {
        if self.shuffle && !self.order.is_empty() {
            self.order
                .iter()
                .position(|&i| i == self.index)
                .unwrap_or(0)
        } else {
            self.index
        }
    }

    /// Indice real de la n-esima entrada del orden efectivo.
    fn nth_in_order(&self, n: usize) -> usize {
        if self.shuffle && !self.order.is_empty() {
            self.order.get(n).copied().unwrap_or(0)
        } else {
            n
        }
    }

    /// Baraja dejando la pista actual la primera, para que activar el modo
    /// aleatorio no corte lo que esta sonando.
    fn reshuffle(&mut self) {
        self.order = (0..self.items.len()).collect();
        if self.order.is_empty() {
            return;
        }
        // Fisher-Yates con una fuente de aleatoriedad barata: no necesitamos
        // calidad criptografica para barajar canciones.
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2545F4914F6CDD1D)
            | 1;
        for i in (1..self.order.len()).rev() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let j = (seed % (i as u64 + 1)) as usize;
            self.order.swap(i, j);
        }
        // La actual, al frente.
        if let Some(p) = self.order.iter().position(|&i| i == self.index) {
            self.order.swap(0, p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> TrackInfo {
        TrackInfo {
            video_id: id.into(),
            title: None,
            author: None,
            thumbnail: None,
            duration_ms: None,
        }
    }

    fn queue_of(n: usize) -> Queue {
        let mut q = Queue::default();
        q.set_items((0..n).map(|i| track(&i.to_string())).collect(), 0);
        q
    }

    #[test]
    fn set_up_next_no_toca_la_pista_actual() {
        // La radio llega cuando la cancion ya suena: si `set_up_next` moviera
        // el indice, la interfaz saltaria de pista sola.
        let mut q = queue_of(3);
        q.jump_to(1);
        let antes = q.current().unwrap().video_id.clone();

        let nuevas = vec![track("x"), track("y")];
        let detras = q.set_up_next(nuevas);

        assert_eq!(q.current().unwrap().video_id, antes, "cambio la pista actual");
        assert_eq!(q.index(), 1);
        assert_eq!(detras, 2);
        assert_eq!(q.len(), 4, "deberia quedar 0,1 + las dos nuevas");
    }

    #[test]
    fn avanza_y_para_al_final() {
        let mut q = queue_of(3);
        assert_eq!(q.next(false).unwrap().video_id, "1");
        assert_eq!(q.next(false).unwrap().video_id, "2");
        assert!(q.next(false).is_none(), "Repeat::Off debe parar al final");
    }

    #[test]
    fn repeat_all_da_la_vuelta() {
        let mut q = queue_of(3);
        q.set_repeat(Repeat::All);
        q.jump_to(2);
        assert_eq!(q.next(false).unwrap().video_id, "0");
    }

    #[test]
    fn repeat_one_solo_afecta_al_fin_natural() {
        let mut q = queue_of(3);
        q.set_repeat(Repeat::One);
        assert_eq!(q.next(true).unwrap().video_id, "0", "fin natural repite");
        assert_eq!(q.next(false).unwrap().video_id, "1", "el usuario si avanza");
    }

    #[test]
    fn prev_se_queda_al_principio() {
        let mut q = queue_of(3);
        assert_eq!(q.prev().unwrap().video_id, "0");
    }

    #[test]
    fn peek_next_no_muta() {
        let mut q = queue_of(3);
        assert_eq!(q.peek_next().unwrap().video_id, "1");
        assert_eq!(q.index(), 0, "peek no debe mover el indice");
    }

    #[test]
    fn shuffle_deja_la_actual_primero() {
        let mut q = queue_of(20);
        q.jump_to(7);
        q.set_shuffle(true);
        assert_eq!(q.current().unwrap().video_id, "7");
        // Y recorre las 20 sin repetir.
        let mut seen = vec![q.current().unwrap().video_id.clone()];
        while let Some(t) = q.next(false) {
            seen.push(t.video_id.clone());
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 20, "shuffle debe cubrir toda la cola");
    }

    /// Nombres de la cola, para que los fallos se lean de un vistazo.
    fn ids(q: &Queue) -> Vec<&str> {
        q.items().iter().map(|t| t.video_id.as_str()).collect()
    }

    #[test]
    fn play_next_conserva_el_resto_de_la_cola() {
        // La diferencia con `set_up_next`, que se lleva por delante todo lo que
        // viene detras. Aqui solo se cuela una.
        let mut q = Queue::default();
        q.set_items(vec![track("a"), track("b"), track("c")], 0);
        q.play_next(track("z"));

        assert_eq!(ids(&q), ["a", "z", "b", "c"]);
        assert_eq!(q.current().unwrap().video_id, "a", "la que suena no cambia");
    }

    #[test]
    fn play_next_de_algo_que_ya_estaba_lo_mueve_en_vez_de_duplicarlo() {
        let mut q = Queue::default();
        q.set_items(vec![track("a"), track("b"), track("c")], 0);
        q.play_next(track("c"));
        assert_eq!(ids(&q), ["a", "c", "b"]);
    }

    #[test]
    fn play_next_no_descoloca_la_actual_al_mover_algo_de_delante() {
        // Sacar una pista anterior a la actual corre el indice: sin ajustarlo,
        // "la que suena" pasaria a ser otra.
        let mut q = Queue::default();
        q.set_items(vec![track("a"), track("b"), track("c")], 2);
        q.play_next(track("a"));

        assert_eq!(ids(&q), ["b", "c", "a"]);
        assert_eq!(q.current().unwrap().video_id, "c", "sigue sonando la misma");
    }

    #[test]
    fn play_next_al_final_de_la_cola() {
        let mut q = Queue::default();
        q.set_items(vec![track("a")], 0);
        q.play_next(track("z"));
        assert_eq!(ids(&q), ["a", "z"]);
    }
}
