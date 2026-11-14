//! Board autosave on a background thread, so serialising and writing a big board never
//! stalls a frame. Only the newest pending copy of each board is written.

use crate::model::{save_board, Board};
use std::collections::HashMap;
use std::sync::{Condvar, Mutex, OnceLock};

struct Q {
    pending: HashMap<String, Board>,
    busy: bool,
    err: Option<String>,
}

fn q() -> &'static (Mutex<Q>, Condvar) {
    static S: OnceLock<(Mutex<Q>, Condvar)> = OnceLock::new();
    S.get_or_init(|| {
        std::thread::spawn(worker);
        (Mutex::new(Q { pending: HashMap::new(), busy: false, err: None }), Condvar::new())
    })
}

fn worker() {
    let (m, cv) = q();
    loop {
        let b = {
            let mut g = m.lock().unwrap_or_else(|e| e.into_inner());
            while g.pending.is_empty() {
                g = cv.wait(g).unwrap_or_else(|e| e.into_inner());
            }
            let id = g.pending.keys().next().cloned().unwrap_or_default();
            g.busy = true;
            g.pending.remove(&id)
        };
        let r = b.as_ref().map(save_board);
        let mut g = m.lock().unwrap_or_else(|e| e.into_inner());
        g.busy = false;
        if let Some(Err(e)) = r {
            g.err = Some(e);
        }
        cv.notify_all();
    }
}

/// Queue a save (replaces any older pending copy of the same board)
pub(crate) fn save_async(b: Board) {
    let (m, cv) = q();
    m.lock().unwrap_or_else(|e| e.into_inner()).pending.insert(b.id.clone(), b);
    cv.notify_all();
}

/// Block until everything queued is on disk (before hiding or quitting)
pub(crate) fn flush() {
    let (m, cv) = q();
    let mut g = m.lock().unwrap_or_else(|e| e.into_inner());
    while !g.pending.is_empty() || g.busy {
        g = cv.wait(g).unwrap_or_else(|e| e.into_inner());
    }
}

/// Last background save error, if any (shown as a toast)
pub(crate) fn take_error() -> Option<String> {
    q().0.lock().unwrap_or_else(|e| e.into_inner()).err.take()
}
