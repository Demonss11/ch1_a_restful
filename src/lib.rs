//! Minimal REST API prototype (ch1_a): simple explicit routes, extensible architecture.

mod error;
mod handlers;
pub mod model;
mod profile;

pub use error::AppError;
pub use model::{AppState, Build, BuildState, BuildTask, Program, Store};

use axum::{routing::get, Router};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use handlers::*;

async fn compiler(mut rx: mpsc::Receiver<BuildTask>, st: AppState) {
    while let Some(task) = rx.recv().await {
        let mut g = st.store.write().await;
        let Some(b) = g.builds.get_mut(&task.build_id) else {
            continue;
        };
        if b.state != BuildState::Queued {
            continue;
        }
        let artifact = compile_artifact(&b.id, &b.slug, b.n);
        b.log.push(format!("compiled artifact {artifact}"));
        b.artifact = Some(artifact);
        b.state = BuildState::Ready;
    }
}

fn compile_artifact(id: &Uuid, slug: &str, n: u32) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    id.hash(&mut h);
    slug.hash(&mut h);
    n.hash(&mut h);
    format!("{:016x}", h.finish())
}

pub async fn build_app() -> (Router, AppState, mpsc::Receiver<BuildTask>) {
    let (tx, rx) = mpsc::channel::<BuildTask>(100);
    let st = AppState {
        store: Arc::new(RwLock::new(Store::default())),
        build_tx: tx,
    };
    let app = router(st.clone());
    (app, st, rx)
}

pub async fn build_app_spawned() -> (Router, AppState) {
    let (app, st, rx) = build_app().await;
    let st_for_worker = st.clone();
    tokio::spawn(async move {
        compiler(rx, st_for_worker).await;
    });
    (app, st)
}

pub fn router(st: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/profile", get(get_profile)) // ← новое
        .route("/programs", get(list_programs).post(create_program))
        .route("/programs/{slug}", get(get_program))
        .route(
            "/programs/{slug}/builds",
            get(list_builds).post(create_build),
        )
        .route("/builds/{id}", get(get_build))
        .fallback(fallback)
        .method_not_allowed_fallback(method_not_allowed)
        .with_state(st)
}
