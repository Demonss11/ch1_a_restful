use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;
use crate::model::*;
use crate::profile;

fn problem_json(status: u16, title: &str, detail: String) -> Response {
    let body = json!({
        "type": format!("https://demo.dev/err/{}", title.to_lowercase().replace(' ', "-")),
        "title": title,
        "status": status,
        "detail": detail,
        "_links": [{"rel": "entry", "method": "GET", "href": "/"}]
    });
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        )],
        Json(body),
    )
        .into_response()
}

pub async fn root() -> impl IntoResponse {
    Json(json!({
        "name": "risk-compiler-a",
        "note": "Minimal prototype: explicit routes, ready for extension",
        "_links": [
            {"rel": "self",     "method": "GET",  "href": "/"},
            {"rel": "profile",  "method": "GET",  "href": "/profile"},
            {"rel": "programs", "method": "GET",  "href": "/programs"}
        ]
    }))
}

/// Семантический профиль API.
pub async fn get_profile() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(profile::MEDIA_TYPE),
    );
    (headers, profile::PROFILE_DOCUMENT).into_response()
}

pub async fn list_programs(State(st): State<AppState>) -> impl IntoResponse {
    let g = st.store.read().await;
    let items: Vec<serde_json::Value> = g
        .order
        .iter()
        .filter_map(|id| g.programs.get(id))
        .map(|p| {
            let cur = p.current.and_then(|id| g.builds.get(&id));
            let mut links = vec![json!({
                "rel": "self", "method": "GET", "href": format!("/programs/{}", p.slug)
            })];
            if let Some(b) = cur {
                links.push(json!({
                    "rel": "current-build", "method": "GET", "href": format!("/builds/{}", b.id)
                }));
            }
            json!({
                "slug": p.slug,
                "title": p.title,
                "state": cur.map(|b| b.state),
                "_links": links
            })
        })
        .collect();
    Json(json!({
        "total": items.len(),
        "items": items,
        "_links": [
            {"rel": "self",           "method": "GET",  "href": "/programs"},
            {"rel": "profile",        "method": "GET",  "href": "/profile"},
            {"rel": "create-program", "method": "POST", "href": "/programs"}
        ]
    }))
}

pub async fn get_program(
    State(st): State<AppState>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let g = st.store.read().await;
    let pid = g.by_slug.get(&slug).ok_or_else(|| AppError::NotFound {
        resource: "Program",
        id: slug.clone(),
    })?;
    let p = g.programs.get(pid).ok_or(AppError::Internal)?;
    Ok(Json(program_rep(&g, p)))
}

pub async fn list_builds(
    State(st): State<AppState>,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let g = st.store.read().await;
    let pid = g.by_slug.get(&slug).ok_or_else(|| AppError::NotFound {
        resource: "Program",
        id: slug.clone(),
    })?;
    let mut items: Vec<&Build> = g.builds.values().filter(|b| b.program == *pid).collect();
    items.sort_by_key(|b| b.n);
    let r: Vec<serde_json::Value> = items
        .iter()
        .map(|b| {
            json!({
                "id": b.id,
                "n": b.n,
                "state": b.state,
                "_links": [{"rel": "self", "method": "GET", "href": format!("/builds/{}", b.id)}]
            })
        })
        .collect();
    Ok(Json(json!({
        "slug": slug,
        "total": r.len(),
        "items": r,
        "_links": [
            {"rel": "self",    "method": "GET", "href": format!("/programs/{slug}/builds")},
            {"rel": "profile", "method": "GET", "href": "/profile"},
            {"rel": "program", "method": "GET", "href": format!("/programs/{slug}")}
        ]
    })))
}

pub async fn create_build(
    State(st): State<AppState>,
    Path(slug): Path<String>,
    payload: Result<Json<CreateBuildReq>, JsonRejection>,
) -> Result<Response, AppError> {
    let Json(req) = payload.map_err(|e| AppError::BadRequest(e.body_text()))?;
    let source = req.source.trim().to_string();
    if source.is_empty() {
        return Err(AppError::BadRequest("source must not be empty".into()));
    }

    let (build, tx) = {
        let mut g = st.store.write().await;
        let pid = g
            .by_slug
            .get(&slug)
            .cloned()
            .ok_or_else(|| AppError::NotFound {
                resource: "Program",
                id: slug.clone(),
            })?;
        let bn = g.builds.values().filter(|b| b.program == pid).count() as u32 + 1;
        let b = Build {
            id: Uuid::new_v4(),
            program: pid,
            n: bn,
            source: source.clone(),
            state: BuildState::Queued,
            log: vec![format!("queued #{bn}")],
            artifact: None,
        };
        g.builds.insert(b.id, b.clone());
        if let Some(p) = g.programs.get_mut(&pid) {
            p.current = Some(b.id);
            p.source = source;
        }
        let rep = build_rep(&g, &b);
        (rep, st.build_tx.clone())
    };

    tx.send(BuildTask { build_id: build.id })
        .await
        .map_err(|_| AppError::Internal)?;

    let location = format!("/builds/{}", build.id);
    Ok((StatusCode::CREATED, [("location", location)], Json(build)).into_response())
}

pub async fn get_build(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let g = st.store.read().await;
    let b = g.builds.get(&id).ok_or_else(|| AppError::NotFound {
        resource: "Build",
        id: id.to_string(),
    })?;
    Ok(Json(build_rep(&g, b)))
}

pub async fn fallback(uri: Uri) -> Response {
    AppError::NotFound {
        resource: "Resource",
        id: uri.path().to_string(),
    }
    .into_response()
}

pub async fn create_program(
    State(st): State<AppState>,
    payload: Result<Json<CreateProgramReq>, JsonRejection>,
) -> Result<Response, AppError> {
    let Json(req) = payload.map_err(|e| AppError::BadRequest(e.body_text()))?;

    validate_slug(&req.slug)?;
    let title = req.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::BadRequest("title must not be empty".into()));
    }
    if title.chars().count() > 200 {
        return Err(AppError::BadRequest(
            "title must be at most 200 chars".into(),
        ));
    }
    let source = req.source.trim().to_string();
    if source.is_empty() {
        return Err(AppError::BadRequest("source must not be empty".into()));
    }
    let slug = req.slug;

    let mut g = st.store.write().await;
    if g.by_slug.contains_key(&slug) {
        return Err(AppError::Conflict(format!(
            "program '{slug}' already exists"
        )));
    }
    let id = Uuid::new_v4();
    let build_id = Uuid::new_v4();
    let build = Build {
        id: build_id,
        program: id,
        n: 1,
        source: source.clone(),
        state: BuildState::Queued,
        log: vec!["queued #1".to_string()],
        artifact: None,
    };
    let p = Program {
        slug: slug.clone(),
        title,
        source,
        current: Some(build_id),
    };
    g.by_slug.insert(slug, id);
    g.order.push(id);
    g.programs.insert(id, p.clone());
    g.builds.insert(build_id, build);
    let rep = program_rep(&g, &p);

    st.build_tx
        .send(BuildTask { build_id })
        .await
        .map_err(|_| AppError::Internal)?;

    Ok((
        StatusCode::CREATED,
        [("location", format!("/programs/{}", rep.slug))],
        Json(rep),
    )
        .into_response())
}

fn validate_slug(s: &str) -> Result<(), AppError> {
    if s.is_empty() {
        return Err(AppError::BadRequest("slug must not be empty".into()));
    }
    if s.len() > 128 {
        return Err(AppError::BadRequest(
            "slug must be at most 128 chars".into(),
        ));
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::BadRequest(
            "slug may contain only ASCII letters, digits, '-' and '_'".into(),
        ));
    }
    Ok(())
}

pub async fn method_not_allowed(method: Method, uri: Uri) -> Response {
    problem_json(
        405,
        "Method Not Allowed",
        format!("{method} not allowed for {}", uri.path()),
    )
}
