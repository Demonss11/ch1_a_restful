use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuildState {
    Queued,
    Ready,
    Failed,
}

impl BuildState {
    pub fn as_str(&self) -> &'static str {
        match self {
            BuildState::Queued => "queued",
            BuildState::Ready => "ready",
            BuildState::Failed => "failed",
        }
    }
}

#[derive(Serialize, Clone)]
pub struct Link {
    pub rel: &'static str,
    pub method: &'static str,
    pub href: String,
}

#[derive(Clone)]
pub struct Program {
    pub slug: String,
    pub title: String,
    pub source: String,
    pub current: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct CreateProgramReq {
    pub slug: String,
    pub title: String,
    pub source: String,
}

#[derive(Deserialize)]
pub struct CreateBuildReq {
    pub source: String,
}

#[derive(Clone)]
pub struct Build {
    pub id: Uuid,
    pub program: Uuid,
    pub n: u32,
    pub source: String,
    pub state: BuildState,
    pub log: Vec<String>,
    pub artifact: Option<String>,
}

#[derive(Serialize)]
pub struct ProgramRep {
    pub slug: String,
    pub title: String,
    pub source: String,
    pub current: Option<Uuid>,
    pub state: Option<BuildState>,
    #[serde(rename = "_links")]
    pub links: Vec<Link>,
}

#[derive(Serialize)]
pub struct BuildRep {
    pub id: Uuid,
    pub program: String,
    pub n: u32,
    pub state: BuildState,
    pub artifact: Option<String>,
    pub log: Vec<String>,
    #[serde(rename = "_links")]
    pub links: Vec<Link>,
}

#[derive(Default)]
pub struct Store {
    pub programs: HashMap<Uuid, Program>,
    pub by_slug: HashMap<String, Uuid>,
    pub order: Vec<Uuid>,
    pub builds: HashMap<Uuid, Build>,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<RwLock<Store>>,
    pub build_tx: mpsc::Sender<BuildTask>,
}

#[derive(Clone, Debug)]
pub struct BuildTask {
    pub build_id: Uuid,
}

pub fn program_rep(store: &Store, p: &Program) -> ProgramRep {
    let cur = p.current.and_then(|id| store.builds.get(&id));
    let mut links = vec![
        Link {
            rel: "self",
            method: "GET",
            href: format!("/programs/{}", p.slug),
        },
        Link {
            rel: "builds",
            method: "GET",
            href: format!("/programs/{}/builds", p.slug),
        },
        Link {
            rel: "create-build",
            method: "POST",
            href: format!("/programs/{}/builds", p.slug),
        },
        Link {
            rel: "profile",
            method: "GET",
            href: "/profile".to_string(),
        },
    ];
    if let Some(b) = cur {
        links.push(Link {
            rel: "current-build",
            method: "GET",
            href: format!("/builds/{}", b.id),
        });
    }
    ProgramRep {
        slug: p.slug.clone(),
        title: p.title.clone(),
        source: p.source.clone(),
        current: p.current,
        state: cur.map(|b| b.state),
        links,
    }
}

/// Репрезентация сборки.
pub fn build_rep(store: &Store, b: &Build) -> BuildRep {
    let slug = store
        .programs
        .get(&b.program)
        .map(|p| p.slug.clone())
        .unwrap_or_default();
    BuildRep {
        id: b.id,
        program: slug.clone(),
        n: b.n,
        state: b.state,
        artifact: b.artifact.clone(),
        log: b.log.clone(),
        links: vec![
            Link {
                rel: "self",
                method: "GET",
                href: format!("/builds/{}", b.id),
            },
            Link {
                rel: "program",
                method: "GET",
                href: format!("/programs/{slug}"),
            },
            Link {
                rel: "profile",
                method: "GET",
                href: "/profile".to_string(),
            },
        ],
    }
}
