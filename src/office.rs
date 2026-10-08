//! Stato condiviso dell'ufficio: chi è in call e la pausa caffè in corso.
//! Le regole seguono quelle di Call Alert, con in più la scadenza delle call "orfane".

use crate::config::now_ms;
use crate::net::Packet;
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

pub struct Call {
    pub user: String,
    pub start_ms: f64,
    pub app: Option<String>,
    pub mine: bool,
    /// Ultima conferma ricevuta (CALL_START o CALL_ALIVE).
    pub seen: Instant,
    /// Vero se il mittente invia CALL_ALIVE (DevLogica Tools): allora la call può scadere.
    pub alive: bool,
}

pub struct MyCall {
    pub id: String,
    pub auto: bool,
    pub app: Option<String>,
    pub start_ms: f64,
}

pub struct Coffee {
    pub by: String,
    pub mine: bool,
    pub rsvp: BTreeMap<String, String>,
    pub since: Instant,
    pub in_minutes: Option<u32>,
}

/// Cosa è cambiato, per decidere cosa mostrare e se suonare.
#[derive(Debug, PartialEq)]
pub enum Notice {
    CallStarted { user: String },
    CallsChanged,
    CoffeeStarted { by: String },
    CoffeeChanged,
}

/// Messaggi da inviare in risposta: `None` = a tutti, `Some(ip)` = solo a quel PC.
pub type Outgoing = Vec<(Option<IpAddr>, Packet)>;

#[derive(Default)]
pub struct Office {
    pub calls: BTreeMap<String, Call>,
    pub my_call: Option<MyCall>,
    pub coffee: Option<Coffee>,
    pub peers: BTreeMap<String, IpAddr>,
}

const ALIVE_TIMEOUT: Duration = Duration::from_secs(75);
const LEGACY_TIMEOUT: Duration = Duration::from_secs(8 * 3600);
const COFFEE_OWNER_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const COFFEE_GUEST_TIMEOUT: Duration = Duration::from_secs(40 * 60);

impl Office {
    /// Gestisce un messaggio arrivato dalla rete.
    pub fn handle(&mut self, p: Packet, from: IpAddr, me: &str) -> (Vec<Notice>, Outgoing) {
        let mut notices = Vec::new();
        let mut out: Outgoing = Vec::new();
        match p {
            Packet::Ping { user } => {
                if let Some(u) = user {
                    self.peers.insert(u, from);
                }
                out.push((Some(from), Packet::Pong { user: Some(me.to_string()), ip: None }));
                if let Some(m) = &self.my_call {
                    out.push((Some(from), Packet::CallStart {
                        call_id: m.id.clone(),
                        user: me.to_string(),
                        start_time: m.start_ms,
                        app: m.app.clone(),
                        auto: m.auto,
                    }));
                }
                if let Some(c) = self.coffee.as_ref().filter(|c| c.mine) {
                    out.push((Some(from), Packet::CoffeeStart { user: me.to_string(), in_minutes: c.in_minutes }));
                    out.push((Some(from), Packet::CoffeeState { rsvp: c.rsvp.clone(), invited_by: Some(me.to_string()) }));
                }
            }
            Packet::Pong { user, .. } => {
                if let Some(u) = user {
                    self.peers.insert(u, from);
                }
            }
            Packet::CallStart { call_id, user, start_time, app, .. } => {
                self.peers.insert(user.clone(), from);
                let new = !self.calls.contains_key(&call_id);
                self.calls.insert(call_id, Call { user: user.clone(), start_ms: start_time, app, mine: false, seen: Instant::now(), alive: false });
                notices.push(if new { Notice::CallStarted { user } } else { Notice::CallsChanged });
            }
            Packet::CallAlive { call_id, user, start_time, app } => {
                let new = !self.calls.contains_key(&call_id);
                let c = self.calls.entry(call_id).or_insert(Call {
                    user: user.clone(),
                    start_ms: start_time,
                    app: app.clone(),
                    mine: false,
                    seen: Instant::now(),
                    alive: true,
                });
                c.seen = Instant::now();
                c.alive = true;
                if c.app.is_none() {
                    c.app = app;
                }
                // Una call che non conoscevamo (es. siamo appena arrivati in rete) va mostrata.
                if new {
                    notices.push(Notice::CallStarted { user });
                }
            }
            Packet::CallEnd { call_id } => {
                if self.calls.remove(&call_id).is_some() {
                    notices.push(Notice::CallsChanged);
                }
            }
            Packet::CoffeeStart { user, in_minutes } => {
                self.peers.insert(user.clone(), from);
                let fresh = self.coffee.as_ref().map(|c| c.by != user).unwrap_or(true);
                if fresh {
                    self.coffee = Some(Coffee { by: user.clone(), mine: false, rsvp: BTreeMap::new(), since: Instant::now(), in_minutes });
                    notices.push(Notice::CoffeeStarted { by: user });
                }
            }
            Packet::CoffeeEnd { .. } => {
                if self.coffee.take().is_some() {
                    notices.push(Notice::CoffeeChanged);
                }
            }
            Packet::CoffeeRsvp { user, answer } => {
                if let Some(c) = self.coffee.as_mut() {
                    c.rsvp.insert(user, answer);
                    // Chi ha proposto il caffè ridistribuisce lo stato aggiornato a tutti.
                    if c.mine {
                        out.push((None, Packet::CoffeeState { rsvp: c.rsvp.clone(), invited_by: Some(me.to_string()) }));
                    }
                    notices.push(Notice::CoffeeChanged);
                }
            }
            Packet::CoffeeState { rsvp, invited_by } => {
                match self.coffee.as_mut() {
                    Some(c) => {
                        if !c.mine {
                            c.rsvp = rsvp;
                        }
                    }
                    None => {
                        // Siamo arrivati a pausa già iniziata.
                        if let Some(by) = invited_by.filter(|b| !b.is_empty()) {
                            self.coffee = Some(Coffee { by: by.clone(), mine: false, rsvp, since: Instant::now(), in_minutes: None });
                            notices.push(Notice::CoffeeStarted { by });
                        }
                    }
                }
                notices.push(Notice::CoffeeChanged);
            }
        }
        (notices, out)
    }

    // ---------- azioni locali: restituiscono il messaggio da inviare a tutti ----------

    pub fn start_my_call(&mut self, me: &str, app: Option<String>, auto: bool) -> Option<Packet> {
        if self.my_call.is_some() {
            return None;
        }
        let start = now_ms();
        let id = format!("{me}-{}", start as u64);
        self.calls.insert(id.clone(), Call { user: me.to_string(), start_ms: start, app: app.clone(), mine: true, seen: Instant::now(), alive: true });
        self.my_call = Some(MyCall { id: id.clone(), auto, app: app.clone(), start_ms: start });
        Some(Packet::CallStart { call_id: id, user: me.to_string(), start_time: start, app, auto })
    }

    pub fn end_my_call(&mut self) -> Option<Packet> {
        let m = self.my_call.take()?;
        self.calls.remove(&m.id);
        Some(Packet::CallEnd { call_id: m.id })
    }

    pub fn alive_packet(&self, me: &str) -> Option<Packet> {
        self.my_call.as_ref().map(|m| Packet::CallAlive { call_id: m.id.clone(), user: me.to_string(), start_time: m.start_ms, app: m.app.clone() })
    }

    pub fn start_coffee(&mut self, me: &str, in_minutes: Option<u32>) -> Packet {
        self.coffee = Some(Coffee { by: me.to_string(), mine: true, rsvp: BTreeMap::new(), since: Instant::now(), in_minutes });
        Packet::CoffeeStart { user: me.to_string(), in_minutes }
    }

    pub fn end_coffee(&mut self, me: &str) -> Option<Packet> {
        match &self.coffee {
            Some(c) if c.mine => {
                self.coffee = None;
                Some(Packet::CoffeeEnd { user: Some(me.to_string()) })
            }
            _ => None,
        }
    }

    pub fn rsvp(&mut self, me: &str, yes: bool) -> Option<Packet> {
        let c = self.coffee.as_mut()?;
        let answer = if yes { "yes" } else { "no" }.to_string();
        c.rsvp.insert(me.to_string(), answer.clone());
        Some(Packet::CoffeeRsvp { user: me.to_string(), answer })
    }

    /// Pulizia periodica. Restituisce eventuali messaggi da inviare e se qualcosa è cambiato.
    pub fn tick(&mut self, me: &str) -> (bool, Vec<Packet>) {
        let mut changed = false;
        let mut out = Vec::new();
        let before = self.calls.len();
        self.calls.retain(|_, c| {
            c.mine || (if c.alive { c.seen.elapsed() < ALIVE_TIMEOUT } else { c.seen.elapsed() < LEGACY_TIMEOUT })
        });
        changed |= before != self.calls.len();
        if let Some(c) = &self.coffee {
            let limit = if c.mine { COFFEE_OWNER_TIMEOUT } else { COFFEE_GUEST_TIMEOUT };
            if c.since.elapsed() > limit {
                if let Some(p) = self.end_coffee(me) {
                    out.push(p);
                }
                self.coffee = None;
                changed = true;
            }
        }
        (changed, out)
    }

    /// Call dei colleghi (escluse le mie), in ordine di inizio.
    pub fn others(&self) -> Vec<&Call> {
        let mut v: Vec<&Call> = self.calls.values().filter(|c| !c.mine).collect();
        v.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
        v
    }

    /// Cambio nome: se ero in call, la call va ri-annunciata con il nome nuovo.
    pub fn rename(&mut self, me: &str) -> Vec<Packet> {
        let mut out = Vec::new();
        if let Some(m) = self.my_call.take() {
            out.push(Packet::CallEnd { call_id: m.id.clone() });
            self.calls.remove(&m.id);
            if let Some(p) = self.start_my_call(me, m.app, m.auto) {
                out.push(p);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn ip(n: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, n))
    }

    #[test]
    fn call_dei_colleghi() {
        let mut o = Office::default();
        let (n, _) = o.handle(Packet::CallStart { call_id: "M-1".into(), user: "Marco".into(), start_time: 1.0, app: Some("Zoom".into()), auto: true }, ip(5), "Ema");
        assert_eq!(n, vec![Notice::CallStarted { user: "Marco".into() }]);
        assert_eq!(o.others().len(), 1);
        assert_eq!(o.peers.get("Marco"), Some(&ip(5)));
        let (n, _) = o.handle(Packet::CallEnd { call_id: "M-1".into() }, ip(5), "Ema");
        assert_eq!(n, vec![Notice::CallsChanged]);
        assert!(o.calls.is_empty());
    }

    #[test]
    fn rispondo_al_ping_con_la_mia_call() {
        let mut o = Office::default();
        o.start_my_call("Ema", Some("Teams".into()), true).unwrap();
        let (_, out) = o.handle(Packet::Ping { user: Some("Anna".into()) }, ip(7), "Ema");
        assert!(out.iter().any(|(to, p)| *to == Some(ip(7)) && matches!(p, Packet::Pong { .. })));
        assert!(out.iter().any(|(to, p)| *to == Some(ip(7)) && matches!(p, Packet::CallStart { .. })));
        assert!(o.end_my_call().is_some());
        assert!(o.end_my_call().is_none());
    }

    #[test]
    fn pausa_caffe() {
        let mut o = Office::default();
        o.start_coffee("Ema", Some(5));
        let (_, out) = o.handle(Packet::CoffeeRsvp { user: "Anna".into(), answer: "yes".into() }, ip(7), "Ema");
        // chi propone ridistribuisce lo stato
        assert!(out.iter().any(|(to, p)| to.is_none() && matches!(p, Packet::CoffeeState { .. })));
        assert_eq!(o.coffee.as_ref().unwrap().rsvp.get("Anna").map(|s| s.as_str()), Some("yes"));
        assert!(o.end_coffee("Ema").is_some());

        // invito ricevuto da un collega
        let (n, _) = o.handle(Packet::CoffeeStart { user: "Marco".into(), in_minutes: None }, ip(5), "Ema");
        assert_eq!(n, vec![Notice::CoffeeStarted { by: "Marco".into() }]);
        assert!(o.rsvp("Ema", false).is_some());
        assert!(o.end_coffee("Ema").is_none()); // non è mia
        o.handle(Packet::CoffeeEnd { user: Some("Marco".into()) }, ip(5), "Ema");
        assert!(o.coffee.is_none());
    }

    #[test]
    fn call_orfane_scadono() {
        let mut o = Office::default();
        o.handle(Packet::CallAlive { call_id: "A-1".into(), user: "Anna".into(), start_time: 1.0, app: None }, ip(7), "Ema");
        o.calls.get_mut("A-1").unwrap().seen -= Duration::from_secs(120);
        let (changed, _) = o.tick("Ema");
        assert!(changed);
        assert!(o.calls.is_empty());
    }
}
