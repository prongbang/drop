// AirDrop-style transfers: every browser that opens the app becomes a peer,
// and a file only ever shows up for the two peers involved in it.
use futures_util::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use warp::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use warp::http::{Response, StatusCode};
use warp::hyper::body::Bytes;
use warp::Filter;

// ponytail: transfers live in RAM and die with the process; spill to a temp dir if you need bigger files
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_TRANSFERS: usize = 50;
const MAX_SIGNAL_BYTES: usize = 64 * 1024;
const MAX_SIGNALS_PER_TRANSFER: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Stage {
    Pending,
    Accepted,
    Declined,
    Ready,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum SignalKind {
    Offer,
    Answer,
    Candidate,
    Fallback,
}

#[derive(Clone, Deserialize)]
struct SignalReq {
    id: u64,
    kind: SignalKind,
    data: serde_json::Value,
}

#[derive(Deserialize)]
struct SignalAckReq {
    id: u64,
    sequence: u64,
}

#[derive(Clone, Serialize)]
struct SignalView {
    id: u64,
    sequence: u64,
    from: String,
    kind: SignalKind,
    data: serde_json::Value,
}

struct QueuedSignal {
    sequence: u64,
    to: String,
    view: SignalView,
}

struct Transfer {
    id: u64,
    from: String,
    to: String,
    // captured when the offer is made, so a peer that leaves is still named on it
    from_name: String,
    to_name: String,
    name: String,
    size: u64,
    mime: String,
    stage: Stage,
    direct: bool,
    relay: bool,
    data: Option<Bytes>,
    signals: Vec<QueuedSignal>,
}

#[derive(Deserialize)]
struct Me {
    me: String,
}

#[derive(Deserialize)]
struct OfferReq {
    to: String,
    name: String,
    size: u64,
    mime: String,
}

#[derive(Deserialize)]
struct RespondReq {
    id: u64,
    accept: bool,
}

#[derive(Serialize)]
struct PeerView {
    id: String,
    name: String,
}

#[derive(Serialize)]
struct TransferView {
    id: u64,
    peer: String,
    name: String,
    size: u64,
    stage: Stage,
    direct: bool,
    relay: bool,
    incoming: bool,
}

#[derive(Serialize)]
struct StateView {
    name: String,
    peers: Vec<PeerView>,
    transfers: Vec<TransferView>,
    signals: Vec<SignalView>,
}

#[derive(Default)]
struct Inner {
    peers: HashMap<String, String>,
    transfers: Vec<Transfer>,
    next_id: u64,
    next_signal: u64,
}

#[derive(Clone)]
pub struct Hub {
    inner: Arc<Mutex<Inner>>,
    changed: broadcast::Sender<()>,
}

impl Default for Hub {
    fn default() -> Self {
        Hub {
            inner: Arc::default(),
            changed: broadcast::channel(16).0,
        }
    }
}

// A peer is present for exactly as long as its event stream is open.
struct Presence {
    hub: Hub,
    id: String,
}

impl Drop for Presence {
    fn drop(&mut self) {
        self.hub.inner.lock().unwrap().peers.remove(&self.id);
        self.hub.announce();
    }
}

// Enough to tell two phones apart in a list; the id keeps it unique.
fn device_name(ua: &str, id: &str) -> String {
    let os = if ua.contains("iPhone") {
        "iPhone"
    } else if ua.contains("iPad") {
        "iPad"
    } else if ua.contains("Android") {
        "Android"
    } else if ua.contains("Macintosh") {
        "Mac"
    } else if ua.contains("Windows") {
        "Windows"
    } else if ua.contains("Linux") {
        "Linux"
    } else {
        "Device"
    };
    let browser = if ua.contains("Edg/") {
        "Edge"
    } else if ua.contains("Firefox/") {
        "Firefox"
    } else if ua.contains("Chrome/") {
        "Chrome"
    } else if ua.contains("Safari/") {
        "Safari"
    } else {
        "browser"
    };
    let tail: String = id.chars().rev().take(4).collect();
    format!("{} · {} · {}", os, browser, tail)
}

impl Hub {
    // Wake every open event stream; they each re-read their own view.
    fn announce(&self) {
        let _ = self.changed.send(());
    }

    fn state(&self, me: &str) -> StateView {
        let inner = self.inner.lock().unwrap();
        StateView {
            name: inner.peers.get(me).cloned().unwrap_or_default(),
            peers: inner
                .peers
                .iter()
                .filter(|(id, _)| id.as_str() != me)
                .map(|(id, name)| PeerView {
                    id: id.clone(),
                    name: name.clone(),
                })
                .collect(),
            transfers: inner
                .transfers
                .iter()
                .filter(|t| t.from == me || t.to == me)
                .map(|t| {
                    let incoming = t.to == me;
                    TransferView {
                        id: t.id,
                        peer: if incoming {
                            t.from_name.clone()
                        } else {
                            t.to_name.clone()
                        },
                        name: t.name.clone(),
                        size: t.size,
                        stage: t.stage,
                        direct: t.direct,
                        relay: t.relay,
                        incoming,
                    }
                })
                .collect(),
            signals: inner
                .transfers
                .iter()
                .flat_map(|transfer| {
                    transfer
                        .signals
                        .iter()
                        .filter(move |signal| signal.to == me)
                        .map(|signal| signal.view.clone())
                })
                .collect(),
        }
    }

    fn join(&self, me: &str, ua: &str) {
        self.inner
            .lock()
            .unwrap()
            .peers
            .insert(me.to_string(), device_name(ua, me));
        self.announce();
    }

    fn offer(&self, me: &str, req: OfferReq) -> Option<u64> {
        let id = {
            let mut inner = self.inner.lock().unwrap();
            if req.size > MAX_FILE_BYTES {
                return None;
            }
            let to_name = inner.peers.get(&req.to)?.clone();
            let from_name = inner.peers.get(me).cloned().unwrap_or_default();
            let id = inner.next_id;
            inner.next_id += 1;
            inner.transfers.push(Transfer {
                id,
                from: me.to_string(),
                to: req.to,
                from_name,
                to_name,
                name: req.name,
                size: req.size,
                mime: req.mime,
                stage: Stage::Pending,
                direct: false,
                relay: false,
                data: None,
                signals: Vec::new(),
            });
            let overflow = inner.transfers.len().saturating_sub(MAX_TRANSFERS);
            inner.transfers.drain(..overflow);
            id
        };
        self.announce();
        Some(id)
    }

    // Only the receiver decides, and only once.
    fn respond(&self, me: &str, req: RespondReq) -> bool {
        let done = {
            let mut inner = self.inner.lock().unwrap();
            match inner.transfers.iter_mut().find(|t| t.id == req.id) {
                Some(t) if t.to == me && t.stage == Stage::Pending => {
                    t.stage = if req.accept {
                        Stage::Accepted
                    } else {
                        Stage::Declined
                    };
                    true
                }
                _ => false,
            }
        };
        if done {
            self.announce();
        }
        done
    }

    // The sender only gets to upload once the receiver has accepted.
    fn upload(&self, me: &str, id: u64, data: Bytes) -> bool {
        let done = {
            let mut inner = self.inner.lock().unwrap();
            match inner.transfers.iter_mut().find(|t| t.id == id) {
                Some(t) if t.from == me && t.stage == Stage::Accepted => {
                    t.size = data.len() as u64;
                    t.data = Some(data);
                    t.stage = Stage::Ready;
                    t.relay = true;
                    true
                }
                _ => false,
            }
        };
        if done {
            self.announce();
        }
        done
    }

    fn signal(&self, me: &str, req: SignalReq) -> Option<u64> {
        if serde_json::to_vec(&req.data).ok()?.len() > MAX_SIGNAL_BYTES {
            return None;
        }
        let sequence = {
            let mut inner = self.inner.lock().unwrap();
            let sequence = inner.next_signal;
            let transfer = inner
                .transfers
                .iter_mut()
                .find(|transfer| transfer.id == req.id)?;
            if transfer.stage != Stage::Accepted
                || transfer.from != me && transfer.to != me
                || transfer.signals.len() >= MAX_SIGNALS_PER_TRANSFER
            {
                return None;
            }
            if req.kind == SignalKind::Fallback {
                transfer.relay = true;
            }
            let to = if transfer.from == me {
                transfer.to.clone()
            } else {
                transfer.from.clone()
            };
            transfer.signals.push(QueuedSignal {
                sequence,
                to,
                view: SignalView {
                    id: req.id,
                    sequence,
                    from: me.to_string(),
                    kind: req.kind,
                    data: req.data,
                },
            });
            inner.next_signal += 1;
            sequence
        };
        self.announce();
        Some(sequence)
    }

    fn ack_signal(&self, me: &str, id: u64, sequence: u64) -> bool {
        let removed = {
            let mut inner = self.inner.lock().unwrap();
            let Some(transfer) = inner
                .transfers
                .iter_mut()
                .find(|transfer| transfer.id == id)
            else {
                return false;
            };
            let Some(index) = transfer
                .signals
                .iter()
                .position(|signal| signal.sequence == sequence && signal.to == me)
            else {
                return false;
            };
            transfer.signals.remove(index);
            true
        };
        if removed {
            self.announce();
        }
        removed
    }

    fn complete(&self, me: &str, id: u64) -> bool {
        let done = {
            let mut inner = self.inner.lock().unwrap();
            match inner
                .transfers
                .iter_mut()
                .find(|transfer| transfer.id == id)
            {
                Some(transfer) if transfer.to == me && transfer.stage == Stage::Accepted => {
                    transfer.stage = Stage::Ready;
                    transfer.direct = true;
                    true
                }
                _ => false,
            }
        };
        if done {
            self.announce();
        }
        done
    }

    fn take(&self, me: &str, id: u64) -> Option<(String, String, Bytes)> {
        let inner = self.inner.lock().unwrap();
        let t = inner.transfers.iter().find(|t| t.id == id)?;
        // a third device that guessed the id is not part of this transfer
        if t.from != me && t.to != me {
            return None;
        }
        Some((t.name.clone(), t.mime.clone(), t.data.clone()?))
    }

    // One state push on connect, then one per change, until the browser goes away.
    fn events(
        &self,
        me: String,
        ua: String,
    ) -> impl Stream<Item = Result<warp::sse::Event, Infallible>> {
        self.join(&me, &ua);
        let seed = (
            self.changed.subscribe(),
            self.clone(),
            me.clone(),
            Presence {
                hub: self.clone(),
                id: me,
            },
            true,
        );
        stream::unfold(seed, |(mut changed, hub, me, presence, first)| async move {
            if !first {
                // a lagged receiver only missed intermediate states; the next one is current
                match changed.recv().await {
                    Ok(()) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
            let event = warp::sse::Event::default().json_data(hub.state(&me)).ok()?;
            Some((Ok(event), (changed, hub, me, presence, false)))
        })
    }
}

pub fn routes(
    hub: Hub,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let hub = warp::any().map(move || hub.clone());
    let ok = |good: bool| {
        warp::reply::with_status(
            warp::reply::json(&good),
            if good {
                StatusCode::OK
            } else {
                StatusCode::CONFLICT
            },
        )
    };

    let events = warp::path!("api" / "events")
        .and(warp::get())
        .and(warp::query::<Me>())
        .and(warp::header::optional::<String>("user-agent"))
        .and(hub.clone())
        .map(|me: Me, ua: Option<String>, hub: Hub| {
            warp::sse::reply(
                warp::sse::keep_alive().stream(hub.events(me.me, ua.unwrap_or_default())),
            )
        });

    let offer = warp::path!("api" / "offer")
        .and(warp::post())
        .and(warp::query::<Me>())
        .and(warp::body::json())
        .and(hub.clone())
        .map(
            |me: Me, req: OfferReq, hub: Hub| match hub.offer(&me.me, req) {
                Some(id) => warp::reply::with_status(warp::reply::json(&id), StatusCode::OK),
                None => warp::reply::with_status(warp::reply::json(&()), StatusCode::CONFLICT),
            },
        );

    let respond = warp::path!("api" / "respond")
        .and(warp::post())
        .and(warp::query::<Me>())
        .and(warp::body::json())
        .and(hub.clone())
        .map(move |me: Me, req: RespondReq, hub: Hub| ok(hub.respond(&me.me, req)));

    let upload = warp::path!("api" / "upload" / u64)
        .and(warp::post())
        .and(warp::query::<Me>())
        .and(warp::body::content_length_limit(MAX_FILE_BYTES))
        .and(warp::body::bytes())
        .and(hub.clone())
        .map(move |id: u64, me: Me, data: Bytes, hub: Hub| ok(hub.upload(&me.me, id, data)));

    let signal = warp::path!("api" / "signal")
        .and(warp::post())
        .and(warp::query::<Me>())
        .and(warp::body::content_length_limit(MAX_SIGNAL_BYTES as u64))
        .and(warp::body::json())
        .and(hub.clone())
        .map(
            |me: Me, req: SignalReq, hub: Hub| match hub.signal(&me.me, req) {
                Some(sequence) => {
                    warp::reply::with_status(warp::reply::json(&sequence), StatusCode::OK)
                }
                None => warp::reply::with_status(warp::reply::json(&()), StatusCode::CONFLICT),
            },
        );

    let signal_ack = warp::path!("api" / "signal" / "ack")
        .and(warp::post())
        .and(warp::query::<Me>())
        .and(warp::body::json())
        .and(hub.clone())
        .map(move |me: Me, req: SignalAckReq, hub: Hub| {
            ok(hub.ack_signal(&me.me, req.id, req.sequence))
        });

    let complete = warp::path!("api" / "complete" / u64)
        .and(warp::post())
        .and(warp::query::<Me>())
        .and(hub.clone())
        .map(move |id: u64, me: Me, hub: Hub| ok(hub.complete(&me.me, id)));

    let download = warp::path!("api" / "transfer" / u64)
        .and(warp::get())
        .and(warp::query::<Me>())
        .and(hub)
        .map(|id: u64, me: Me, hub: Hub| match hub.take(&me.me, id) {
            Some((name, mime, data)) => Response::builder()
                .header(CONTENT_TYPE, mime)
                .header(
                    CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", name),
                )
                .body(data)
                .unwrap(),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Bytes::new())
                .unwrap(),
        });

    events
        .or(offer)
        .or(respond)
        .or(upload)
        .or(signal)
        .or(signal_ack)
        .or(complete)
        .or(download)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    fn offer_req(to: &str) -> OfferReq {
        OfferReq {
            to: to.into(),
            name: "note.txt".into(),
            size: 3,
            mime: "text/plain".into(),
        }
    }

    #[test]
    fn a_transfer_is_private_to_its_two_peers() {
        let hub = Hub::default();
        for peer in ["a", "b", "c"] {
            hub.join(peer, "iPhone Safari");
        }

        let id = hub.offer("a", offer_req("b")).expect("b is online");
        assert!(hub.offer("a", offer_req("nobody")).is_none());

        // nothing moves until the receiver accepts
        assert!(!hub.upload("a", id, Bytes::from("hi")));
        assert!(
            !hub.respond("c", RespondReq { id, accept: true }),
            "only b decides"
        );
        assert!(hub.respond("b", RespondReq { id, accept: true }));
        assert!(hub.upload("a", id, Bytes::from("hi")));

        assert!(hub.take("b", id).is_some());
        assert!(hub.take("a", id).is_some());
        assert!(hub.state("b").transfers[0].relay);
        assert!(hub.take("c", id).is_none(), "a bystander cannot fetch it");
        assert_eq!(
            hub.state("c").transfers.len(),
            0,
            "and never sees it listed"
        );
        assert_eq!(hub.state("b").transfers.len(), 1);
    }

    #[test]
    fn declining_keeps_the_file_from_ever_being_sent() {
        let hub = Hub::default();
        hub.join("a", "");
        hub.join("b", "");
        let id = hub.offer("a", offer_req("b")).unwrap();
        assert!(hub.respond("b", RespondReq { id, accept: false }));
        assert!(!hub.upload("a", id, Bytes::from("hi")));
        assert!(hub.take("b", id).is_none());
    }

    #[test]
    fn signaling_is_visible_only_to_the_other_transfer_participant() {
        let hub = Hub::default();
        for peer in ["a", "b", "c"] {
            hub.join(peer, "");
        }
        let id = hub.offer("a", offer_req("b")).unwrap();
        hub.respond("b", RespondReq { id, accept: true });

        let sequence = hub
            .signal(
                "a",
                SignalReq {
                    id,
                    kind: SignalKind::Offer,
                    data: serde_json::json!({ "sdp": "offer" }),
                },
            )
            .unwrap();

        assert_eq!(hub.state("b").signals.len(), 1);
        assert_eq!(hub.state("b").signals[0].sequence, sequence);
        assert_eq!(hub.state("c").signals.len(), 0);
        assert!(hub
            .signal(
                "c",
                SignalReq {
                    id,
                    kind: SignalKind::Answer,
                    data: serde_json::json!({ "sdp": "intrusion" }),
                }
            )
            .is_none());
        assert!(hub
            .signal(
                "b",
                SignalReq {
                    id,
                    kind: SignalKind::Fallback,
                    data: serde_json::json!({ "reason": "unsupported" }),
                }
            )
            .is_some());
        assert!(hub.state("a").transfers[0].relay);
    }

    #[test]
    fn receiver_can_complete_a_direct_transfer_but_sender_cannot() {
        let hub = Hub::default();
        hub.join("a", "");
        hub.join("b", "");
        let id = hub.offer("a", offer_req("b")).unwrap();
        hub.respond("b", RespondReq { id, accept: true });

        assert!(!hub.complete("a", id));
        assert_eq!(hub.state("b").transfers[0].stage, Stage::Accepted);
        assert!(hub.complete("b", id));
        assert_eq!(hub.state("b").transfers[0].stage, Stage::Ready);
        assert!(hub.state("b").transfers[0].direct);
    }

    #[test]
    fn receiver_acknowledges_a_signal_once() {
        let hub = Hub::default();
        hub.join("a", "");
        hub.join("b", "");
        let id = hub.offer("a", offer_req("b")).unwrap();
        hub.respond("b", RespondReq { id, accept: true });
        let sequence = hub
            .signal(
                "a",
                SignalReq {
                    id,
                    kind: SignalKind::Offer,
                    data: serde_json::json!({ "sdp": "offer" }),
                },
            )
            .unwrap();

        assert!(hub.ack_signal("b", id, sequence));
        assert!(!hub.ack_signal("b", id, sequence));
        assert!(hub.state("b").signals.is_empty());
    }

    #[tokio::test]
    async fn the_stream_pushes_on_change_and_presence_ends_with_it() {
        let hub = Hub::default();
        let mut events = Box::pin(hub.events("a".into(), "Macintosh Chrome/1".into()));

        // the first push is the current state, without waiting for anything to happen
        assert!(events.next().await.is_some());

        hub.join("b", "iPhone Safari");
        assert!(events.next().await.is_some(), "b joining wakes a");
        assert_eq!(
            hub.state("b").peers.len(),
            1,
            "b sees a while a is streaming"
        );

        drop(events);
        assert_eq!(
            hub.state("b").peers.len(),
            0,
            "a is gone when its stream is"
        );
    }
}
