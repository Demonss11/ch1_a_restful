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

pub async fn root() -> impl IntoResponse {
    Json(json!({
        "name": "risk-compiler-a",
        "note": "Minimal prototype: explicit routes, ready for extension",
        "_links": [
            {"rel": "self",           "method": "GET",  "href": "/"},
            {"rel": "profile",        "method": "GET",  "href": "/profile"},
            {"rel": "programs",       "method": "GET",  "href": "/programs"},
            {"rel": "create-program", "method": "POST", "href": "/programs"}
        ]
    }))
}

/// Семантический профиль API. Возвращается с собственным медиатипом,
/// чтобы клиент мог отличить его от «обычных» JSON-ответов и привязаться
/// именно к нему (а не к схеме объекта).
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
            json!({
                "slug": p.slug,
                "title": p.title,
                "_links": [
                    {"rel": "self", "method": "GET", "href": format!("/programs/{}", p.slug)}
                ]
            })
        })
        .collect();
    Json(json!({
        "total": items.len(),
        "items": items,
        "_links": [{"rel": "self", "method": "GET", "href": "/programs"}]
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
        "_links": [{"rel": "self", "method": "GET", "href": format!("/programs/{slug}/builds")}]
    })))
}

pub async fn create_build(
    State(st): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Response, AppError> {
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
            slug: slug.clone(),
            n: bn,
            state: BuildState::Queued,
            log: vec![format!("queued #n{bn}")],
            artifact: None,
        };
        g.builds.insert(b.id, b.clone());
        (b, st.build_tx.clone())
    };

    tx.send(BuildTask { build_id: build.id })
        .await
        .map_err(|_| AppError::Internal)?;

    Ok((StatusCode::CREATED, Json(build_rep(&build))).into_response())
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
    Ok(Json(build_rep(b)))
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
    // Единый формат ошибок: реджект Json-экстрактора конвертируем в AppError.
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
    let slug = req.slug;

    let mut g = st.store.write().await;
    if g.by_slug.contains_key(&slug) {
        return Err(AppError::Conflict(format!(
            "program '{slug}' already exists"
        )));
    }
    let id = Uuid::new_v4();
    let p = Program {
        slug: slug.clone(),
        title,
        current: None,
    };
    g.by_slug.insert(slug, id);
    g.order.push(id);
    g.programs.insert(id, p.clone());
    Ok((StatusCode::CREATED, Json(program_rep(&g, &p))).into_response())
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
    let body = serde_json::json!({
        "type": "https://demo.dev/err/method-not-allowed",
        "title": "Method Not Allowed",
        "status": 405,
        "detail": format!("{method} not allowed for {}", uri.path()),
        "_links": [{"rel": "entry", "method": "GET", "href": "/"}]
    });
    (StatusCode::METHOD_NOT_ALLOWED, Json(body)).into_response()
}
