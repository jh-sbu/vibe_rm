//! Cached, asynchronous timing for engines that do not own an audio device.
//! Asset reads and codec work both belong to the worker, never the simulation thread.
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
};

struct Worker {
    request: Sender<String>,
    results: Receiver<(String, Option<f32>)>,
}
impl Worker {
    fn spawn(load: impl Fn(&str) -> Option<f32> + Send + 'static) -> std::io::Result<Self> {
        let (request, rx) = mpsc::channel::<String>();
        let (tx, results) = mpsc::channel();
        std::thread::Builder::new()
            .name("audio-duration".into())
            .spawn(move || {
                while let Ok(path) = rx.recv() {
                    let duration = load(&path);
                    if tx.send((path, duration)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self { request, results })
    }
}
#[derive(Default)]
pub struct Cache {
    values: HashMap<String, Option<f32>>,
    pending: HashSet<String>,
    worker: Option<Worker>,
}
impl Cache {
    pub fn request(&mut self, vfs: &Arc<vfs::Vfs>, path: &str) -> String {
        let path = vfs::normalize_path(path);
        self.poll();
        if self.values.contains_key(&path) || self.pending.contains(&path) {
            return path;
        }
        if self.worker.is_none() {
            let vfs = Arc::clone(vfs);
            self.worker = Worker::spawn(move |path| {
                let bytes = vfs.read(path)?;
                super::decode::decode(path, &bytes, 44100).map(|c| c.duration())
            })
            .map_err(|e| log::warn!("audio duration worker: {e}"))
            .ok();
        }
        if self
            .worker
            .as_ref()
            .is_some_and(|w| w.request.send(path.clone()).is_ok())
        {
            self.pending.insert(path.clone());
        } else {
            self.worker_failed();
            self.values.insert(path.clone(), None);
        }
        path
    }
    fn finish(&mut self, path: String, duration: Option<f32>) {
        self.pending.remove(&path);
        self.values.insert(path, duration);
    }
    fn worker_failed(&mut self) {
        self.worker = None;
        for path in self.pending.drain() {
            self.values.insert(path, None);
        }
    }
    pub fn poll(&mut self) {
        while let Some(worker) = self.worker.as_ref() {
            match worker.results.try_recv() {
                Ok((path, duration)) => self.finish(path, duration),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.worker_failed();
                    break;
                }
            }
        }
    }
    /// None means pending; Some(None) is a cached missing/undecodable asset.
    pub fn get(&self, path: &str) -> Option<Option<f32>> {
        self.values.get(path).copied()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn worker_disconnect_resolves_every_pending_path_and_allows_restart() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = Worker::spawn(move |_| {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            panic!("synthetic corrupt asset");
        })
        .unwrap();
        let mut cache = Cache {
            worker: Some(worker),
            ..Default::default()
        };
        cache.values.insert("completed.wav".into(), Some(3.0));
        let vfs = Arc::new(vfs::Vfs::new(
            std::env::temp_dir().join("vrm-unused-duration-source"),
            &[],
        ));
        cache.request(&vfs, "bad.xwm");
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        cache.request(&vfs, "queued.xwm");
        release_tx.send(()).unwrap();
        assert!(matches!(
            cache
                .worker
                .as_ref()
                .unwrap()
                .results
                .recv_timeout(Duration::from_secs(2)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
        cache.poll();
        assert!(cache.worker.is_none());
        assert!(cache.pending.is_empty());
        assert_eq!(cache.get("bad.xwm"), Some(None));
        assert_eq!(cache.get("queued.xwm"), Some(None));
        assert_eq!(cache.get("completed.wav"), Some(Some(3.0)));
        cache.request(&vfs, "new.wav");
        assert!(cache.worker.is_some());
        let result = cache
            .worker
            .as_ref()
            .unwrap()
            .results
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        cache.finish(result.0, result.1);
        assert_eq!(cache.get("new.wav"), Some(None));
    }
    #[test]
    fn slow_timing_work_is_nonblocking_deduplicated_and_cached() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = Worker::spawn(move |path| {
            started_tx.send(path.to_string()).unwrap();
            release_rx.recv().unwrap();
            Some(2.0)
        })
        .unwrap();
        let mut cache = Cache {
            worker: Some(worker),
            ..Default::default()
        };
        let vfs = Arc::new(vfs::Vfs::new(
            std::env::temp_dir().join("vrm-unused-duration-source"),
            &[],
        ));
        // This returns even while the worker is deliberately blocked.
        let path = cache.request(&vfs, "Music/Track.XWM");
        assert_eq!(path, "music/track.xwm");
        assert_eq!(
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            path
        );
        assert_eq!(cache.get(&path), None);
        assert_eq!(cache.request(&vfs, &path), path);
        release_tx.send(()).unwrap();
        let result = cache
            .worker
            .as_ref()
            .unwrap()
            .results
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        cache.finish(result.0, result.1);
        assert_eq!(cache.get(&path), Some(Some(2.0)));
        cache.request(&vfs, &path);
        assert!(started_rx.try_recv().is_err());
    }
    #[test]
    fn failed_assets_are_cached_as_failures() {
        let mut cache = Cache {
            worker: Some(Worker::spawn(|_| None).unwrap()),
            ..Default::default()
        };
        let vfs = Arc::new(vfs::Vfs::new(
            std::env::temp_dir().join("vrm-unused-duration-source"),
            &[],
        ));
        let path = cache.request(&vfs, "missing.xwm");
        let result = cache
            .worker
            .as_ref()
            .unwrap()
            .results
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        cache.finish(result.0, result.1);
        cache.request(&vfs, &path);
        assert_eq!(cache.get(&path), Some(None));
        assert!(cache.pending.is_empty());
    }
}
