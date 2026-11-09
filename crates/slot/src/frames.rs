use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Video handoff from the emulator thread to the renderer. Buffers are moved, never copied,
/// and the lock is only ever held for a pointer swap, so neither side waits on the other.
/// Three buffers are enough: one being written, one published, one being read.
pub struct Frames {
    inner: Mutex<Inner>,
    size: usize,
    /// Every successful `latest`. `latest` consumes, so anything that calls it outside the
    /// render path silently steals a frame the renderer would have drawn.
    taken: AtomicU64,
}

struct Inner {
    /// The published frame, its width and its height. The size travels with the bytes
    /// because since the arcade cores landed it is not a property of the machine alone: an
    /// FBNeo cart hands over whatever frame its board produces (384x224 for CPS, 256x224 for
    /// others), so the picture's own dimensions have to reach the renderer with the pixels.
    /// Before this it was implied by whichever `System` was active, which only worked while
    /// every machine had exactly one frame size.
    ready: Option<(Vec<u8>, u32, u32)>,
    spare: Vec<Vec<u8>>,
    allocated: usize,
}

impl Frames {
    pub fn new(size: usize) -> Arc<Self> {
        Arc::new(Frames {
            inner: Mutex::new(Inner {
                ready: None,
                spare: Vec::new(),
                allocated: 0,
            }),
            size,
            taken: AtomicU64::new(0),
        })
    }

    pub fn take_write(&self) -> Vec<u8> {
        let mut i = self.lock();
        match i.spare.pop() {
            Some(buf) => buf,
            None => {
                i.allocated += 1;
                Vec::with_capacity(self.size)
            }
        }
    }

    /// A frame the renderer never picked up is overwritten rather than queued. Presenting a
    /// stale frame late is worse than never presenting it. `w`/`h` are the frame's own
    /// dimensions, carried through to `FrameRef` so the upload never has to guess them.
    pub fn publish(&self, buf: Vec<u8>, w: u32, h: u32) {
        let mut i = self.lock();
        if let Some((dropped, _, _)) = i.ready.replace((buf, w, h)) {
            i.spare.push(dropped);
        }
    }

    pub fn latest(self: &Arc<Self>) -> Option<FrameRef> {
        let (buf, w, h) = self.lock().ready.take()?;
        self.taken.fetch_add(1, Ordering::Relaxed);
        Some(FrameRef {
            frames: self.clone(),
            buf,
            w,
            h,
        })
    }

    /// Non consuming. The only safe way to ask whether a frame is waiting.
    pub fn is_ready(&self) -> bool {
        self.lock().ready.is_some()
    }

    pub fn taken(&self) -> u64 {
        self.taken.load(Ordering::Relaxed)
    }

    pub fn allocated(&self) -> usize {
        self.lock().allocated
    }

    fn recycle(&self, buf: Vec<u8>) {
        self.lock().spare.push(buf);
    }

    /// A poisoned lock means one side panicked mid swap. The worst case is a lost frame,
    /// which is better than the renderer giving up for the rest of the session.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct FrameRef {
    frames: Arc<Frames>,
    buf: Vec<u8>,
    w: u32,
    h: u32,
}

impl FrameRef {
    /// The frame's own width in pixels — what the upload must be told, and what the picture
    /// rect is worked against. Not the active machine's nominal size: the two agree for every
    /// fixed-frame machine and differ for the arcade boards.
    pub fn width(&self) -> u32 {
        self.w
    }

    pub fn height(&self) -> u32 {
        self.h
    }
}

impl Deref for FrameRef {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buf
    }
}

impl Drop for FrameRef {
    fn drop(&mut self) {
        self.frames.recycle(std::mem::take(&mut self.buf));
    }
}