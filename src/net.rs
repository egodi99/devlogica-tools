//! Rete locale: broadcast UDP sulla porta 47832, stesso protocollo JSON di Call Alert,
//! così le due app possono convivere durante il passaggio.

use crate::config::log;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

pub const PORT: u16 = 47832;

/// Messaggi scambiati tra i PC. I nomi e i campi coincidono con quelli di Call Alert;
/// i campi in più (`app`, `auto`, `inMinutes`) vengono semplicemente ignorati da Call Alert.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Packet {
    #[serde(rename = "PING")]
    Ping {
        #[serde(default)]
        user: Option<String>,
    },
    #[serde(rename = "PONG")]
    Pong {
        #[serde(default)]
        user: Option<String>,
        #[serde(default)]
        ip: Option<String>,
    },
    #[serde(rename = "CALL_START", rename_all = "camelCase")]
    CallStart {
        call_id: String,
        user: String,
        start_time: f64,
        /// App con cui si è in call (es. "Microsoft Teams"). Solo DevLogica Tools.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app: Option<String>,
        /// Vero se rilevata in automatico dal microfono. Solo DevLogica Tools.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        auto: bool,
    },
    /// Conferma periodica che la call è ancora in corso (Call Alert non lo conosce e lo ignora).
    /// Serve a far sparire le call di un PC che si è spento o ha perso la rete.
    #[serde(rename = "CALL_ALIVE", rename_all = "camelCase")]
    CallAlive {
        call_id: String,
        user: String,
        start_time: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app: Option<String>,
    },
    #[serde(rename = "CALL_END", rename_all = "camelCase")]
    CallEnd { call_id: String },
    #[serde(rename = "COFFEE_START", rename_all = "camelCase")]
    CoffeeStart {
        user: String,
        /// Tra quanti minuti (assente = adesso). Solo DevLogica Tools.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        in_minutes: Option<u32>,
    },
    #[serde(rename = "COFFEE_END")]
    CoffeeEnd {
        #[serde(default)]
        user: Option<String>,
    },
    #[serde(rename = "COFFEE_RSVP")]
    CoffeeRsvp { user: String, answer: String },
    #[serde(rename = "COFFEE_STATE", rename_all = "camelCase")]
    CoffeeState {
        #[serde(default)]
        rsvp: BTreeMap<String, String>,
        #[serde(default)]
        invited_by: Option<String>,
    },
}

pub struct Net {
    sock: UdpSocket,
}

/// Indirizzi IPv4 di questo PC (escluso il loopback).
pub fn local_ips() -> Vec<Ipv4Addr> {
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| match i.addr {
            if_addrs::IfAddr::V4(v4) if !v4.ip.is_loopback() => Some(v4.ip),
            _ => None,
        })
        .collect()
}

/// Indirizzi di broadcast di ogni sottorete (più affidabili di 255.255.255.255,
/// che molti router e firewall bloccano), più quello globale come riserva.
fn broadcast_addrs() -> Vec<Ipv4Addr> {
    let mut v = vec![Ipv4Addr::BROADCAST];
    for i in if_addrs::get_if_addrs().unwrap_or_default() {
        if let if_addrs::IfAddr::V4(a) = i.addr {
            if a.ip.is_loopback() || a.ip.is_link_local() {
                continue;
            }
            let b = a.broadcast.unwrap_or_else(|| {
                Ipv4Addr::from(u32::from(a.ip) | !u32::from(a.netmask))
            });
            if !v.contains(&b) {
                v.push(b);
            }
        }
    }
    v
}

impl Net {
    /// Apre la porta e avvia il thread di ricezione: ogni messaggio arriva a `on_packet`.
    pub fn start(on_packet: impl Fn(Packet, IpAddr) + Send + 'static) -> Option<Self> {
        use socket2::{Domain, Protocol, Socket, Type};
        let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).ok()?;
        // Riutilizzo della porta: permette di avere Call Alert aperto sullo stesso PC.
        let _ = s.set_reuse_address(true);
        #[cfg(unix)]
        let _ = s.set_reuse_port(true);
        let _ = s.set_broadcast(true);
        let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, PORT));
        if let Err(e) = s.bind(&addr.into()) {
            log(format!("Rete: impossibile aprire la porta UDP {PORT}: {e}"));
            return None;
        }
        let sock: UdpSocket = s.into();
        let rx = sock.try_clone().ok()?;
        std::thread::Builder::new()
            .name("rete".into())
            .spawn(move || {
                let mut buf = vec![0u8; 64 * 1024];
                let mut mine = local_ips();
                let mut refreshed = std::time::Instant::now();
                loop {
                    let Ok((n, from)) = rx.recv_from(&mut buf) else { continue };
                    // I nostri stessi broadcast tornano indietro: li scartiamo.
                    if refreshed.elapsed().as_secs() > 30 {
                        mine = local_ips();
                        refreshed = std::time::Instant::now();
                    }
                    if let IpAddr::V4(v4) = from.ip() {
                        if mine.contains(&v4) || v4.is_loopback() {
                            continue;
                        }
                    }
                    match serde_json::from_slice::<Packet>(&buf[..n]) {
                        Ok(p) => on_packet(p, from.ip()),
                        Err(_) => {} // tipo di messaggio sconosciuto: ignorato
                    }
                }
            })
            .ok()?;
        log(format!("Rete: in ascolto su UDP {PORT}, IP locali {:?}", local_ips()));
        Some(Self { sock })
    }

    fn bytes(p: &Packet) -> Vec<u8> {
        serde_json::to_vec(p).unwrap_or_default()
    }

    /// Invia a tutta la rete locale e, in unicast, ai colleghi già noti.
    pub fn broadcast(&self, p: &Packet, peers: &BTreeMap<String, IpAddr>) {
        let data = Self::bytes(p);
        for b in broadcast_addrs() {
            let _ = self.sock.send_to(&data, (b, PORT));
        }
        let mut sent: Vec<IpAddr> = Vec::new();
        for ip in peers.values() {
            if !sent.contains(ip) {
                let _ = self.sock.send_to(&data, (*ip, PORT));
                sent.push(*ip);
            }
        }
    }

    pub fn send_to(&self, p: &Packet, ip: IpAddr) {
        let _ = self.sock.send_to(&Self::bytes(p), (ip, PORT));
    }
}

#[cfg(test)]
mod tests {
    use super::Packet;

    /// I messaggi esattamente come li invia Call Alert devono essere letti correttamente.
    #[test]
    fn legge_call_alert() {
        let p: Packet = serde_json::from_str(r#"{"type":"CALL_START","callId":"Marco-1728390000000","user":"Marco","startTime":1728390000000}"#).unwrap();
        assert!(matches!(p, Packet::CallStart { ref user, auto: false, app: None, .. } if user == "Marco"));
        let p: Packet = serde_json::from_str(r#"{"type":"PONG","user":"Anna","ip":"192.168.1.20"}"#).unwrap();
        assert!(matches!(p, Packet::Pong { .. }));
        let p: Packet = serde_json::from_str(r#"{"type":"COFFEE_STATE","rsvp":{"Anna":"yes"},"invitedBy":"Marco"}"#).unwrap();
        assert!(matches!(p, Packet::CoffeeState { ref invited_by, .. } if invited_by.as_deref() == Some("Marco")));
        let p: Packet = serde_json::from_str(r#"{"type":"COFFEE_END","user":"Marco"}"#).unwrap();
        assert!(matches!(p, Packet::CoffeeEnd { .. }));
        assert!(serde_json::from_str::<Packet>(r#"{"type":"QUALCOSA_DI_NUOVO"}"#).is_err());
    }

    /// I messaggi inviati devono avere i nomi dei campi che Call Alert si aspetta.
    #[test]
    fn scrive_per_call_alert() {
        let s = serde_json::to_string(&Packet::CallStart { call_id: "E-1".into(), user: "E".into(), start_time: 1.0, app: None, auto: false }).unwrap();
        assert_eq!(s, r#"{"type":"CALL_START","callId":"E-1","user":"E","startTime":1.0}"#);
        let s = serde_json::to_string(&Packet::CallEnd { call_id: "E-1".into() }).unwrap();
        assert_eq!(s, r#"{"type":"CALL_END","callId":"E-1"}"#);
        let s = serde_json::to_string(&Packet::CoffeeRsvp { user: "E".into(), answer: "yes".into() }).unwrap();
        assert_eq!(s, r#"{"type":"COFFEE_RSVP","user":"E","answer":"yes"}"#);
        let s = serde_json::to_string(&Packet::CoffeeStart { user: "E".into(), in_minutes: Some(5) }).unwrap();
        assert_eq!(s, r#"{"type":"COFFEE_START","user":"E","inMinutes":5}"#);
    }
}
