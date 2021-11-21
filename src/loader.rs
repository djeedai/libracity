use bevy::prelude::*;
use parking_lot::{Mutex, RwLock};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    /// Idle state where a [`Loader`] is ready to receive new requests.
    Ready,
    /// Loading state during which the [`Loader`] actively work with the asset server
    /// to load the group of assets.
    Loading,
    /// Final state indicating the group of assets has been loaded.
    Done,
}

/// Helper to load a group of assets together and wait for completion of all without
/// having to manually poll for each asset individually.
/// 
/// # Lifecycle
///
/// The loader starts in an idle state where requests can be enqueued with [`enqueue`].
/// Once all requests are made, calling [`submit`] starts the actual loading via the
/// asset server. The loading state of the entire group can be queried with [`is_done`];
/// once that returns `true`, individual assets can be extracted from the [`Loader`]
/// with [`take`].
/// 
/// The [`Loader`] will keep all assets loaded until they're consume with [`take`], or
/// the loader is reset with [`reset`]. When reset, all pending and loaded assets are
/// forgotten (the asset server may continue pending loadings, but the loader will not
/// keep a handle to them). Once reset, a new batch of assets can be enqueued and submitted,
/// allowing to reuse the loader for a subsequent operation.
/// 
/// # Example
/// 
/// ```rust
/// // Create the loader and enqueue requests, generally from a startup system.
/// fn setup(mut commands: Commands) {
///   let mut loader = Loader::new();
///   loader.enqueue("texture.png");
///   loader.enqueue("mesh.gltf");
///   loader.submit();
///   commands.spawn().insert(loader);
/// }
///
/// // Poll the loader for completion, and consume the loaded assets.
/// fn update() {
///   if loader.is_done() {
///     let tex_handle = loader.take("texture.png").unwrap();
///     let mesh_handle = loader.take("mesh.gltf").unwrap();
///     loader.reset(); // ensures is_done() returns false next time
///   }
/// }
/// ```
/// 
/// [`enqueue`]: Loader::enqueue
/// [`submit`]: Loader::submit
/// [`is_done`]: Loader::is_done
/// [`take`]: Loader::take
/// [`reset`]: Loader::reset
#[derive(Debug)]
pub struct Loader {
    /// Loader state.
    state: RwLock<State>,
    /// Number of pending load requests that did not complete yet.
    count: AtomicUsize,
    /// Request queue containing the assets not yet queried to the asset server.
    request_queue: Mutex<Vec<String>>,
    /// Work queue for assets being loaded by the asset server.
    work_queue: Mutex<Vec<(String, HandleUntyped)>>,
    /// Completion queue keeping assets loaded after they're removed from the work queue.
    complete_queue: Mutex<HashMap<String, HandleUntyped>>,
}

impl Loader {
    pub fn new() -> Self {
        Loader {
            state: RwLock::new(State::Ready),
            count: AtomicUsize::new(0),
            request_queue: Mutex::new(vec![]),
            work_queue: Mutex::new(vec![]),
            complete_queue: Mutex::new(HashMap::new()),
        }
    }

    /// Reset the loader to its idle state. This allows submitting a new batch of asset loading requests.
    /// All pending requests and already loaded assets are forgotten. If the assets were already loaded,
    /// and were not consumed with [`take`], the last reference may be dropped and they may get unloaded
    /// by the asset server.
    ///
    /// [`take`]: Loader::take
    pub fn reset(&mut self) {
        let mut state = self.state.write();
        if *state != State::Ready {
            self.request_queue.lock().clear();
            self.work_queue.lock().clear();
            self.count.store(0, Ordering::Release);
            self.complete_queue.lock().clear();
            *state = State::Ready;
        }
    }

    /// Enqueue a new asset loading request.
    ///
    /// Panics if the loader is not in the idle state.
    pub fn enqueue(&mut self, path: &str) {
        assert!(*self.state.read() == State::Ready);
        self.request_queue.lock().push(path.to_owned());
        self.count.fetch_add(1, Ordering::Release);
        trace!(
            "Enqueued request: {} ({}/{})",
            path,
            self.request_queue.lock().len(),
            self.count.load(Ordering::Relaxed)
        );
    }

    /// Submit the pending batch of asset loading requests. After this, no new request can be
    /// enqueued until [`reset`] is called.
    /// 
    /// Panics if the loader is not in the idle state.
    /// 
    /// [`reset`]: Loader::reset
    pub fn submit(&mut self) {
        let mut state = self.state.write();
        assert!(*state == State::Ready);
        *state = State::Loading;
    }

    /// Is the loader empty? Returns `true` if there is no pending asset loading request.
    pub fn is_empty(&self) -> bool {
        self.pending_count() == 0
    }

    /// Number of pending asset loading requests.
    pub fn pending_count(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }

    /// Is the loader done loading the current asset batch?
    pub fn is_done(&self) -> bool {
        *self.state.read() == State::Done
    }

    /// Check if the asset with the given path was loaded already.
    pub fn is_loaded(&self, path: &str) -> bool {
        self.complete_queue.lock().contains_key(path)
    }

    /// Take the asset with the given path, if found and loaded, and remove its handle from the loader.
    /// After this, the loader will forget about that asset and not keep it loaded anymore.
    pub fn take(&mut self, path: &str) -> Option<HandleUntyped> {
        self.complete_queue.lock().remove(path)
    }

    fn tick(&mut self, asset_server: &AssetServer) {
        // Check pending asset loading requests and remove completed ones
        {
            let mut work_queue = self.work_queue.lock();
            // TODO - Vec::drain_filter()
            let mut i = 0;
            while i < work_queue.len() {
                let (path, handle) = &work_queue[i];
                let state = asset_server.get_load_state(handle);
                if state == bevy::asset::LoadState::Loaded
                    || state == bevy::asset::LoadState::Failed
                {
                    trace!("Asset finished loading: {} {:?}", path, handle);
                    let (path, handle) = work_queue.remove(i);
                    self.complete_queue.lock().insert(path, handle);
                    if self.count.fetch_sub(1, Ordering::Acquire) == 1 {
                        // Last asset loaded, all done
                        *self.state.write() = State::Done;
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
                    self.work_queue.lock().push((path, handle));
                }
                bevy::asset::LoadState::Loaded | bevy::asset::LoadState::Failed => {
                    trace!("Asset: {} -> {:?}", path, &handle);
                    self.count.fetch_sub(1, Ordering::Release);
                }
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
        let mut loader = Loader::new();
        loader.submit();
        assert!(loader.is_empty());
        assert_eq!(loader.pending_count(), 0);
    }

    #[test]
    fn enqueue() {
        let mut loader = Loader::new();
        loader.enqueue("dummy");
        loader.submit();
        assert!(!loader.is_empty());
        assert_eq!(loader.pending_count(), 1);
        //let asset_server = AssetServer::new(asset_io, task_queue);
        //loader.work(&asset_server);
    }
}
