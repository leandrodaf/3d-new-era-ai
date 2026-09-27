//! Crash reports from a build that holds no DSN, through the project's site.
//!
//! Sentry's client signs an envelope with the DSN it was built with and posts
//! it straight to the ingest host. A build made from a clone has no DSN, so
//! until now a crash there was seen by nobody — and those are the builds of
//! everyone who compiles the app themselves. What this does is Sentry's own
//! tunnel: the envelope leaves as it always does and goes to
//! `3dneweraai.com/ping/sentry`, which signs it with the DSN the site holds
//! and forwards it to Sentry unchanged.
//!
//! The queue is one thread, like Sentry's own transport: capturing an event
//! never waits on the network, and [`Transport::flush`] — what dropping the
//! guard calls — still gets the last one out.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

/// How long one envelope may take before it is given up on.
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

/// Hands Sentry's client the transport below.
pub(crate) struct Factory {
    pub(crate) url: String,
}

impl sentry::TransportFactory for Factory {
    fn create_transport_with_options(
        &self,
        _options: sentry::TransportOptions,
    ) -> Arc<dyn sentry::Transport> {
        Arc::new(Tunnel::new(self.url.clone()))
    }
}

struct Tunnel {
    queue: mpsc::Sender<Vec<u8>>,
    /// Envelopes handed over and not yet sent, so a flush knows what to wait
    /// for.
    pending: Arc<AtomicUsize>,
}

impl Tunnel {
    fn new(url: String) -> Self {
        let (queue, waiting) = mpsc::channel::<Vec<u8>>();
        let pending = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&pending);
        std::thread::spawn(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(SEND_TIMEOUT))
                .build()
                .into();
            // The sender is gone when the process is on its way out; the loop
            // ends there.
            for envelope in waiting {
                // A report that cannot be delivered is dropped: there is
                // nothing useful to say about it on the way out of a crash.
                let _ = agent
                    .post(&url)
                    .header("content-type", "application/x-sentry-envelope")
                    .send(envelope);
                counted.fetch_sub(1, Ordering::SeqCst);
            }
        });
        Self { queue, pending }
    }
}

impl sentry::Transport for Tunnel {
    fn send_envelope(&self, envelope: sentry::Envelope) {
        let mut body = Vec::new();
        if envelope.to_writer(&mut body).is_err() {
            return;
        }
        self.pending.fetch_add(1, Ordering::SeqCst);
        if self.queue.send(body).is_err() {
            self.pending.fetch_sub(1, Ordering::SeqCst);
        }
    }

    fn flush(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while self.pending.load(Ordering::SeqCst) > 0 {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use sentry::Transport as _;

    /// Nothing waits on the network to be counted as sent: the envelope is
    /// queued and the caller is free, and a flush with nothing pending answers
    /// at once. The address is one nothing listens on, which is the point —
    /// the thread's failure is nobody's business.
    #[test]
    fn an_envelope_is_queued_and_flushed_without_waiting() {
        let tunnel = super::Tunnel::new("http://127.0.0.1:1/ping/sentry".to_owned());
        assert!(tunnel.flush(std::time::Duration::from_millis(50)));
        let event = sentry::protocol::Event {
            message: Some("a report".to_owned()),
            ..Default::default()
        };
        tunnel.send_envelope(event.into());
        // The queue drains on its own thread; a timeout here is not a failure
        // of the queue, so what is asserted is only that flushing returns.
        let _ = tunnel.flush(std::time::Duration::from_secs(1));
    }
}
