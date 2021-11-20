use bevy::prelude::*;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct Loader {
    count: AtomicUsize,
    request_queue: Mutex<Vec<String>>,
    work_queue: Mutex<Vec<HandleUntyped>>,
}

impl Loader {
    pub fn new() -> Self {
        Loader {
            count: AtomicUsize::new(0),
            request_queue: Mutex::new(vec![]),
            work_queue: Mutex::new(vec![]),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pending_count() == 0
    }

    pub fn pending_count(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }

    pub fn is_done(&self) -> bool {
        // Keep request queue locked while reading count to avoid race condition
        self.is_empty()
    }

    pub fn enqueue(&mut self, path: &str) {
        self.request_queue.lock().push(path.to_owned());
        self.count.fetch_add(1, Ordering::Release);
        trace!("Enqueued request: {} ({}/{})", path, self.request_queue.lock().len(), self.count.load(Ordering::Relaxed));
    }

    pub fn tick(&mut self, asset_server: &AssetServer) {
        // Check pending asset loading requests and remove completed ones
        {
            let mut work_queue = self.work_queue.lock();
            // TODO - Vec::drain_filter()
            let mut i = 0;
            while i < work_queue.len() {
                let handle = &work_queue[i];
                let state = asset_server.get_load_state(handle);
                if state == bevy::asset::LoadState::Loaded
                    || state == bevy::asset::LoadState::Failed
                {
                    trace!("Asset finished loading: {:?}", handle);
                    work_queue.remove(i);
                    if self.count.fetch_sub(1, Ordering::Acquire) == 1 {
                        // Finished last loading
                    }
                } else {
                    i += 1;
                }
            }
        }

        // Swap request queue atomically
        let mut request_queue: Vec<String> = {
            let mut request_queue = self.request_queue.lock();
            std::mem::replace(&mut request_queue, vec![])
        };
        
        // Drain request queue and enqueue new asset loading requests
        for path in request_queue.drain(..) {
            let handle = asset_server.load_untyped(&path[..]);
            // Only enqueue if not loaded; otherwise either the resource is already loading
            // (need to wait), is loaded (nothing to do), or failed (no point retrying).
            match asset_server.get_load_state(&handle) {
                bevy::asset::LoadState::NotLoaded | bevy::asset::LoadState::Loading => {
                    trace!("Start loading asset: {} -> {:?}", path, &handle);
                    self.work_queue.lock().push(handle);
                },
                bevy::asset::LoadState::Loaded | bevy::asset::LoadState::Failed => {
                    trace!("Asset: {} -> {:?}", path, &handle);
                    self.count.fetch_sub(1, Ordering::Release);
                },
            }
        }
    }
}

fn tick_loaders(asset_server: Res<AssetServer>, mut query: Query<(&mut Loader,)>) {
    let asset_server: &AssetServer = &*asset_server;
    for (mut loader,) in query.iter_mut() {
        loader.tick(asset_server);
    }
}

pub struct LoaderPlugin;

static LOADER_STAGE: &str = "loader";

impl Plugin for LoaderPlugin {
    fn build(&self, app: &mut AppBuilder) {
        // Add Level resource and event
        app.add_stage_before(
            CoreStage::First,
            LOADER_STAGE,
            SystemStage::single_threaded(),
        )
        .add_system_to_stage(LOADER_STAGE, tick_loaders.system());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let loader = Loader::new();
        assert!(loader.is_empty());
        assert_eq!(loader.pending_count(), 0);
    }

    #[test]
    fn enqueue() {
        let mut loader = Loader::new();
        loader.enqueue("dummy");
        assert!(!loader.is_empty());
        assert_eq!(loader.pending_count(), 1);
        //let asset_server = AssetServer::new(asset_io, task_queue);
        //loader.work(&asset_server);
    }
}
