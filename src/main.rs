use askama::Template;
use axum::{
    extract::{Form, Path, Query, State},
//    extract::{Form, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post, put},
    Router,
};
use anyhow::Context;
use dotenvy::dotenv;
use serde::Deserialize;
use sqlx::{FromRow, PgPool, postgres::PgPoolOptions, QueryBuilder};
use std::sync::Arc;
use axum::http::{header, HeaderValue};
use tower_http::{services::ServeDir, set_header::SetResponseHeader, trace::TraceLayer};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, FromRow)]
pub struct Model {
    pub id: i32,
    pub name: String,
    pub name_short: String,
    pub description: String,
    pub comments: String,
    pub num_of_times: i32,
    pub num_of_times_short: i32,
    pub score: i32,
    pub score_short: i32,
    pub image_name: String,
    pub long_url: String,
    pub short_url: String,
}

impl Model {
    fn long_status(&self) -> VideoStatus { VideoStatus::of_url(&self.long_url) }
    fn short_status(&self) -> VideoStatus { VideoStatus::of_url(&self.short_url) }
}

const VIDEO_DIR: &str = "static/video";

/// Where a video URL points and, for local files, whether Dropbox has the data on disk.
#[derive(Debug, Clone, Copy, PartialEq)]
enum VideoStatus { Offline, OnlineOnly, Missing, Remote }

impl VideoStatus {
    /// Status of the local copy of a video URL. Dropbox links whose file isn't
    /// in static/video (e.g. on Shuttle) are `Remote`.
    fn of_url(url: &str) -> Self {
        let local = video_file_name(url).map(|f| Self::of_path(&std::path::Path::new(VIDEO_DIR).join(f)));
        match local {
            Some(Self::Missing) | None if is_plain_https(url) => Self::Remote,
            Some(status) => status,
            None => Self::Missing,
        }
    }

    fn of_path(path: &std::path::Path) -> Self {
        match std::fs::metadata(path) {
            Ok(m) if is_dataless(&m) => Self::OnlineOnly,
            Ok(_) => Self::Offline,
            Err(_) => Self::Missing,
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            Self::Offline => "💾",
            Self::OnlineOnly => "☁️",
            Self::Missing => "⚠️",
            Self::Remote => "🌐",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::OnlineOnly => "online-only",
            Self::Missing => "missing",
            Self::Remote => "dropbox link",
        }
    }
}

/// Dropbox (File Provider) marks online-only files as "dataless" until they are downloaded.
#[cfg(target_os = "macos")]
fn is_dataless(m: &std::fs::Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    const SF_DATALESS: u32 = 0x4000_0000;
    m.st_flags() & SF_DATALESS != 0
}

#[cfg(not(target_os = "macos"))]
fn is_dataless(_: &std::fs::Metadata) -> bool { false }

struct VideoFile {
    name: String,
    size: String,
    status: VideoStatus,
    used_by: String,
}

#[derive(Clone)]
struct AppState {
    pool: Arc<PgPool>,
}

type HandlerResult = Result<Html<String>, RequestError>;

#[derive(Debug)]
struct RequestError {
    status: StatusCode,
    body: Html<String>,
    target: String,
    swap: &'static str,
}

impl RequestError {
    fn message(status: StatusCode, message: &'static str) -> Self {
        Self {
            status,
            body: Html(format!("<p role=\"alert\">{message}</p>")),
            target: "#request-error".into(),
            swap: "innerHTML",
        }
    }

    fn database(operation: &'static str, error: sqlx::Error) -> Self {
        error!(operation, error = %error, "database operation failed");
        Self::message(StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load models. Please try again.")
    }

    fn not_found() -> Self {
        Self::message(StatusCode::NOT_FOUND, "This model no longer exists. Refresh the list.")
    }
}

impl IntoResponse for RequestError {
    fn into_response(self) -> Response {
        // Only responses marked by this application may be swapped on an HTTP error.
        (self.status, [
            ("x-app-error", "true"),
            ("hx-retarget", self.target.as_str()),
            ("hx-reswap", self.swap),
            ("hx-push-url", "false"),
        ], self.body).into_response()
    }
}

fn render(template: impl Template) -> HandlerResult {
    template.render().map(Html).map_err(|error| {
        error!(error = %error, "template rendering failed");
        RequestError::message(StatusCode::INTERNAL_SERVER_ERROR,
            "Could not display this page. Please try again.")
    })
}

fn create_form_error(status: StatusCode, form: &ModelForm, message: &'static str) -> HandlerResult {
    let body = render(FormTpl {
        action: "/models",
        method: "post",
        submit_label: "Create",
        target_id: "rows",
        swap_mode: "beforeend",
        values: form,
        error: message,
    })?;
    Err(RequestError { status, body, target: "#modal".into(), swap: "outerHTML" })
}

fn update_form_error(status: StatusCode, form: &ModelForm, message: &'static str) -> HandlerResult {
    let body = render(FormRowTpl { values: form, error: message })?;
    Err(RequestError {
        status, body, target: format!("#row-{}", form.id.unwrap()), swap: "outerHTML",
    })
}

#[derive(Debug, Clone, Deserialize)]
struct ListParams {
    q: Option<String>,
    min_score: Option<i32>,
    sort: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ModelForm {
    id: Option<i32>, // only for edit
    name: String,
    name_short: String,
    description: String,
    comments: String,
    num_of_times: i32,
    num_of_times_short: i32,
    score: i32,
    score_short: i32,
    image_name: String,
    long_url: String,
    short_url: String,
}

#[derive(Template)]
#[template(path = "models.html")]
struct ModelsTpl<'a> {
    title: &'a str,
    items: &'a [Model],
    q_value: &'a str,
    min_score_value: &'a str,
    sort_value: &'a str,
}

#[derive(Template)]
#[template(path = "_rows.html")]
struct RowsTpl<'a> { items: &'a [Model] }

#[derive(Template)]
#[template(path = "_row.html")]
struct RowTpl<'a> { m: &'a Model }
#[derive(Template)]
#[template(path = "_form_row.html")]
struct FormRowTpl<'a> {
    values: &'a ModelForm,
    error: &'a str,
}
#[derive(Template)]
#[template(path = "detail.html")]
struct DetailTpl<'a> { title: &'a str, item: &'a Model }

#[derive(Template)]
#[template(path = "_form.html")]
struct FormTpl<'a> {
    action: &'a str,         // e.g. "/models" or "/models/123"
    method: &'a str,         // "post" or "put"
    submit_label: &'a str,   // "Create" or "Save"
    target_id: &'a str,      // e.g. "rows" or "row-123"
    swap_mode: &'a str,      // "beforeend" (create) or "outerHTML" (edit)
    values: &'a ModelForm,   // your form values (ok to keep Option<i32> here; we won't render it)
    error: &'a str,
}

#[derive(Template)]
#[template(path = "videos.html")]
struct VideosTpl<'a> {
    title: &'a str,
    files: &'a [VideoFile],
    offline_count: usize,
}

#[derive(Deserialize)]
struct VideoParams { kind: Option<String> } // "long" | "short"

#[derive(Template)]
#[template(path = "_video_row.html")]
struct VideoRowTpl {
    id: i32,
    poster: String,
    src: String,
    title: String,
    colspan: usize,}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // .env is optional; real environment variables take precedence.
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set (see .env.example)")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("could not connect to DATABASE_URL")?;

    // Migrations are embedded at compile time, so the binary doesn't need the migrations/ folder.
    sqlx::migrate!().run(&pool).await.context("running migrations")?;

    let state = AppState { pool: Arc::new(pool)};
    let router = build_router(state);

    // Hosting platforms usually pass PORT and need HOST=0.0.0.0; locally only listen on 127.0.0.1.
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".into());
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}"))
        .await
        .with_context(|| format!("binding {host}:{port}"))?;
    info!("listening on http://{}", listener.local_addr()?);

    axum::serve(listener, router).await?;
    Ok(())
}
fn build_router(state: AppState) -> Router {
    /*    let static_svc = get_service(ServeDir::new("static"))
            .handle_error(|err: std::io::Error| async move {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("static file error: {err}"))
            });
    */
    Router::new()
        .route("/", get(models_list))
        .route("/models", get(models_list).post(model_create))
        .route("/models/new", get(model_new_form))
        .route("/models/{id}", get(model_detail).put(model_update).delete(model_delete))
        .route("/models/{id}/edit", get(model_edit_form))
        .route("/models/{id}/video", get(model_video_row))
        .route("/videos", get(videos_list))
        .with_state(state)
        // no-cache: browsers revalidate every time (cheap 304 if unchanged), so CSS/JS edits show up immediately.
        .nest_service("/static", SetResponseHeader::overriding(
            ServeDir::new("static"),
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ))
    //.layer(TraceLayer::new_for_http())
}

// LIST + FILTER
async fn models_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListParams>,
) -> HandlerResult {
    let mut qb = QueryBuilder::new(
        "SELECT id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name, long_url, short_url FROM models"
    );

    let mut sep = " WHERE ";
    if let Some(q) = params.q.as_ref().filter(|s| !s.is_empty()) {
        qb.push(sep)
            .push(" (name ILIKE ").push_bind(format!("%{q}%"))
            .push(" OR name_short ILIKE ").push_bind(format!("%{q}%"))
            .push(" OR description ILIKE ").push_bind(format!("%{q}%"))
            .push(") ");
        sep = " AND ";
    }
    if let Some(min) = params.min_score {
        qb.push(sep).push(" score >= ").push_bind(min);
    }

    qb.push(" ORDER BY ");
    match params.sort.as_deref() {
        Some("name_asc") => qb.push(" name ASC "),
        Some("name_desc") => qb.push(" name DESC "),
        Some("score_asc") => qb.push(" score ASC "),
        Some("score_desc") => qb.push(" score DESC "),
        Some("times_asc") => qb.push(" num_of_times ASC "),
        Some("times_desc") => qb.push(" num_of_times DESC "),
        _ => qb.push(" id ASC "),
    };

    let items: Vec<Model> = qb.build_query_as().fetch_all(&*state.pool).await
        .map_err(|error| RequestError::database("list models", error))?;

    let is_hx = headers.get("HX-Request").is_some();
    let hx_target = headers
        .get("HX-Target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Only send the fragment when the request is explicitly targeting the table body
    if is_hx && hx_target == "rows" {
        return render(RowsTpl { items: &items });
    }

    // otherwise, render full page (so headers appear)
    let q_value = params.q.unwrap_or_default();
    let min_score_value = params.min_score.map(|v| v.to_string()).unwrap_or_default();
    let sort_value = params.sort.unwrap_or_default();

    render(ModelsTpl {
        title: "Qigong Videos 2",
        items: &items,
        q_value: &q_value,
        min_score_value: &min_score_value,
        sort_value: &sort_value,
    })
}
// DETAIL
async fn model_detail(State(state): State<AppState>, Path(id): Path<i32>) -> HandlerResult {
    let item = sqlx::query_as::<_, Model>(
        "SELECT id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name, long_url, short_url FROM models WHERE id=$1"
    ).bind(id).fetch_optional(&*state.pool).await
        .map_err(|error| RequestError::database("load model", error))?;

    match item {
        Some(item) => render(DetailTpl { title: "Model Detail", item: &item }),
        None => Err(RequestError::not_found()),
    }
}

// NEW FORM (fragment)
async fn model_new_form() -> HandlerResult {
    let empty = ModelForm::default();
    render(FormTpl {
        action: "/models",
        method: "post",
        submit_label: "Create",
        target_id: "rows",          // ✅ where to insert the new <tr>
        swap_mode: "beforeend",     // ✅ append to #rows
        values: &empty,
        error: "",
    })
}

// CREATE (returns a single <tr> to append or errors as form)
async fn model_create(
    State(state): State<AppState>,
    Form(form): Form<ModelForm>,
) -> HandlerResult {
    // naive validation
    if form.name.trim().is_empty() {
        return create_form_error(StatusCode::UNPROCESSABLE_ENTITY, &form, "Name is required");
    }

    let q = r#"
        INSERT INTO models (name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name, long_url, short_url)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        RETURNING id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name, long_url, short_url
    "#;

    let result = sqlx::query_as::<_, Model>(q)
        .bind(&form.name)
        .bind(&form.name_short)
        .bind(&form.description)
        .bind(&form.comments)
        .bind(form.num_of_times)
        .bind(form.num_of_times_short)
        .bind(form.score)
        .bind(form.score_short)
        .bind(&form.image_name)
        .bind(&form.long_url)
        .bind(&form.short_url)
        .fetch_one(&*state.pool).await;

    match result {
        Ok(item) => render(RowTpl { m: &item }),
        Err(error) => {
            error!(error = %error, "could not create model");
            create_form_error(StatusCode::INTERNAL_SERVER_ERROR, &form,
                "Could not create the model. Your entries are preserved. Please try again.")
        }
    }
}

// EDIT FORM (fragment)
async fn model_edit_form(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> HandlerResult {
    let item: Option<Model> = sqlx::query_as(
        "SELECT id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name, long_url, short_url FROM models WHERE id=$1"
    ).bind(id).fetch_optional(&*state.pool).await
        .map_err(|error| RequestError::database("load model for editing", error))?;

    if let Some(i) = item {
        let values = ModelForm {
            id: Some(i.id),
            name: i.name,
            name_short: i.name_short,
            description: i.description,
            comments: i.comments,
            num_of_times: i.num_of_times,
            num_of_times_short: i.num_of_times_short,
            score: i.score,
            score_short: i.score_short,
            image_name: i.image_name,
            long_url: i.long_url,
            short_url: i.short_url
        };
        render(FormRowTpl { values: &values, error: "" })
    } else {
        Err(RequestError::not_found())
    }
}

// UPDATE (returns the updated <tr>)
async fn model_update(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Form(mut form): Form<ModelForm>,
) -> HandlerResult {
    // The route identifies the row; the submitted form must not redirect a retry.
    form.id = Some(id);
    if form.name.trim().is_empty() {
        return update_form_error(StatusCode::UNPROCESSABLE_ENTITY, &form, "Name is required");
    }
    let q = r#"
        UPDATE models SET
          name=$1, name_short=$2, description=$3, comments=$4,
          num_of_times=$5, num_of_times_short=$6, score=$7, score_short=$8, image_name=$9, long_url=$10, short_url=$11
        WHERE id=$12
        RETURNING id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name,long_url,short_url
    "#;

    let result = sqlx::query_as::<_, Model>(q)
        .bind(&form.name)
        .bind(&form.name_short)
        .bind(&form.description)
        .bind(&form.comments)
        .bind(form.num_of_times)
        .bind(form.num_of_times_short)
        .bind(form.score)
        .bind(form.score_short)
        .bind(&form.image_name)
        .bind(&form.long_url)
        .bind(&form.short_url)
        .bind(id)
        .fetch_optional(&*state.pool).await;

    match result {
        Ok(Some(item)) => render(RowTpl { m: &item }),
        Ok(None) => update_form_error(StatusCode::NOT_FOUND, &form,
            "This model no longer exists. Copy your entries before refreshing the list."),
        Err(error) => {
            error!(id, error = %error, "could not update model");
            update_form_error(StatusCode::INTERNAL_SERVER_ERROR, &form,
                "Could not save the model. Your entries are preserved. Please try again.")
        }
    }
}

// DELETE (returns an empty string; htmx will remove the row)
async fn model_delete(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> HandlerResult {
    let result = sqlx::query("DELETE FROM models WHERE id=$1").bind(id)
        .execute(&*state.pool).await.map_err(|error| {
            error!(id, error = %error, "could not delete model");
            RequestError::message(StatusCode::INTERNAL_SERVER_ERROR,
                "Could not delete the model. It is still in the list. Please try again.")
        })?;
    if result.rows_affected() == 0 {
        return Err(RequestError::not_found());
    }
    Ok(Html(String::new()))
}

async fn model_video_row(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Query(params): Query<VideoParams>,
) -> HandlerResult {
    let item = sqlx::query_as::<_, Model>(
        "SELECT id,name,name_short,description,comments,
                num_of_times,num_of_times_short,score,score_short,image_name,long_url,short_url
         FROM models WHERE id=$1"
    )
        .bind(id)
        .fetch_optional(&*state.pool)
        .await
        .map_err(|error| RequestError::database("load model video", error))?;

    if let Some(m) = item {
        let use_short = matches!(params.kind.as_deref(), Some("short"));
        let src = if use_short { m.short_url.clone() } else { m.long_url.clone() };
        // ensure normalized before feeding to the template (in case legacy rows exist)
        let src = playback_src(&src);

        // let use_short = matches!(params.kind.as_deref(), Some("short"));
        // let filename = if use_short {
        //     format!("{}.mp4", m.name_short)
        // } else {
        //     format!("{}.mp4", m.name)
        // };
        // let src = format!("/static/video/{}", filename);

        let title = if use_short {
            format!("{} — short", m.name_short)
        } else {
            format!("{} — long", m.name)
        };
        let poster = format!("/static/img/{}", m.image_name);

        return render(VideoRowTpl {
            id: m.id,
            poster,
            src,
            title,
            colspan: 8,
        });
    }

    Err(RequestError::not_found())
}
// LOCAL VIDEO FILES (offline status of every .mp4 in static/video)
async fn videos_list(State(state): State<AppState>) -> HandlerResult {
    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT name, long_url, short_url FROM models ORDER BY id")
            .fetch_all(&*state.pool)
            .await
            .map_err(|error| RequestError::database("list video usage", error))?;

    let mut files: Vec<VideoFile> = std::fs::read_dir(VIDEO_DIR)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.to_ascii_lowercase().ends_with(".mp4"))
        .map(|name| {
            let path = std::path::Path::new(VIDEO_DIR).join(&name);
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let used_by = rows
                .iter()
                .flat_map(|(model, long, short)| {
                    [(long, "long"), (short, "short")]
                        .into_iter()
                        .filter(|(u, _)| video_file_name(u) == Some(name.as_str()))
                        .map(move |(_, kind)| format!("{model} ({kind})"))
                })
                .collect::<Vec<_>>()
                .join(", ");
            VideoFile {
                status: VideoStatus::of_path(&path),
                size: format!("{:.2} GB", bytes as f64 / 1e9),
                name,
                used_by,
            }
        })
        .collect();
    files.sort_by(|a, b| a.name.cmp(&b.name));

    let offline_count = files.iter().filter(|f| f.status == VideoStatus::Offline).count();

    render(VideosTpl { title: "Local videos", files: &files, offline_count })
}

/// File name a video URL refers to: the last path segment of a Dropbox link
/// (`…/April01.mp4?rlkey=…`) or of a `/static/video/April01.mp4` path.
fn video_file_name(url: &str) -> Option<&str> {
    let path = url.split(['?', '#']).next()?;
    let file = path.rsplit('/').next()?;
    file.to_ascii_lowercase().ends_with(".mp4").then_some(file)
}

/// Play the local copy when Dropbox has it offline on this machine, otherwise
/// stream the URL itself. On Shuttle there is no static/video, so Dropbox is used.
fn playback_src(url: &str) -> String {
    match video_file_name(url) {
        Some(file) if VideoStatus::of_path(&std::path::Path::new(VIDEO_DIR).join(file)) == VideoStatus::Offline => {
            format!("/static/video/{file}")
        }
        _ => normalize_dropbox_url(url).unwrap_or_else(|_| url.to_string()),
    }
}

fn normalize_dropbox_url(u: &str) -> Result<String, String> {
    // Accept any https URL; for Dropbox, ensure it streams
    if !u.starts_with("http://") && !u.starts_with("https://") {
        return Err("URL must start with http:// or https://".into());
    }
    if u.contains("dropbox.com") {
        // common: turn preview into raw stream
        if u.contains("dl=0") { return Ok(u.replace("dl=0", "raw=1")); }
        if u.contains("dl=1") { return Ok(u.replace("dl=1", "raw=1")); }
        if u.contains("raw=1") { return Ok(u.to_string()); }
        // no query? append raw=1
        if u.contains('?') { return Ok(format!("{u}&raw=1")); }
        return Ok(format!("{u}?raw=1"));
    }
    Ok(u.to_string())
}

fn looks_like_mp4(u: &str) -> bool {
    u.to_ascii_lowercase().contains(".mp4")
}

fn is_plain_https(u: &str) -> bool {
    u.starts_with("https://") || u.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn error_body(result: HandlerResult, status: StatusCode, target: &str, swap: &str) -> String {
        let response = result.unwrap_err().into_response();
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()["x-app-error"], "true");
        assert_eq!(response.headers()["hx-retarget"], target);
        assert_eq!(response.headers()["hx-reswap"], swap);
        assert_eq!(response.headers()["hx-push-url"], "false");
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    fn rejected_form() -> ModelForm {
        ModelForm {
            id: Some(999),
            name: "Rejected <model>".into(),
            name_short: "Short name".into(),
            description: "Keep my description".into(),
            comments: "Keep my comments".into(),
            image_name: "image.png".into(),
            long_url: "https://example.com/long.mp4".into(),
            short_url: "https://example.com/short.mp4".into(),
            num_of_times: 11,
            num_of_times_short: 12,
            score: 13,
            score_short: 14,
        }
    }

    fn assert_entries_preserved(body: &str) {
        for value in ["Short name", "Keep my description", "Keep my comments", "image.png",
            "https://example.com/long.mp4", "https://example.com/short.mp4",
            "value=\"11\"", "value=\"12\"", "value=\"13\"", "value=\"14\""] {
            assert!(body.contains(value), "missing preserved value: {value}");
        }
        assert!(body.contains("Rejected"));
        assert!(!body.contains("Rejected <model>"), "input must be HTML escaped");
        assert!(!body.contains("secret_database_details"));
    }

    #[sqlx::test(migrations = false)]
    async fn failed_writes_preserve_forms_and_rows_and_allow_retry(pool: PgPool) {
        prepare_id_sequence(&pool, Some(1000)).await;
        sqlx::raw_sql("\
            ALTER TABLE models ADD CONSTRAINT secret_database_details
                CHECK (name <> 'Rejected <model>');
            CREATE FUNCTION reject_delete() RETURNS trigger LANGUAGE plpgsql AS $$
                BEGIN RAISE EXCEPTION 'secret_database_details'; END;
            $$;
            CREATE TRIGGER reject_delete BEFORE DELETE ON models
                FOR EACH ROW EXECUTE FUNCTION reject_delete();
        ").execute(&pool).await.unwrap();
        let state = AppState { pool: Arc::new(pool) };

        let result = model_create(State(state.clone()), Form(rejected_form())).await;
        let body = error_body(result, StatusCode::INTERNAL_SERVER_ERROR, "#modal", "outerHTML").await;
        assert_entries_preserved(&body);
        assert!(body.contains("Could not create"));
        assert!(body.contains("hx-post=\"/models\""));

        let result = model_update(State(state.clone()), Path(1), Form(rejected_form())).await;
        let body = error_body(result, StatusCode::INTERNAL_SERVER_ERROR, "#row-1", "outerHTML").await;
        assert_entries_preserved(&body);
        assert!(body.contains("Could not save"));
        assert!(body.contains("hx-put=\"/models/1\""));
        assert!(body.starts_with("<tr id=\"row-1\">"));
        let name: String = sqlx::query_scalar("SELECT name FROM models WHERE id = 1")
            .fetch_one(&*state.pool).await.unwrap();
        assert_eq!(name, "April01");

        let result = model_delete(State(state.clone()), Path(1)).await;
        let body = error_body(result, StatusCode::INTERNAL_SERVER_ERROR, "#request-error", "innerHTML").await;
        assert!(body.contains("Could not delete"));
        assert!(!body.contains("secret_database_details"));
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM models WHERE id = 1)")
            .fetch_one(&*state.pool).await.unwrap();
        assert!(exists);

        sqlx::raw_sql("\
            DROP TRIGGER reject_delete ON models;
            ALTER TABLE models DROP CONSTRAINT secret_database_details;
        ").execute(&*state.pool).await.unwrap();
        let Html(created) = model_create(State(state.clone()), Form(rejected_form())).await.unwrap();
        assert!(created.starts_with("<tr id=\"row-"));
        let Html(updated) = model_update(State(state.clone()), Path(1), Form(rejected_form())).await.unwrap();
        assert!(updated.starts_with("<tr id=\"row-1\">"));
        let Html(body) = model_delete(State(state.clone()), Path(1)).await.unwrap();
        assert!(body.is_empty());
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM models WHERE id = 1)")
            .fetch_one(&*state.pool).await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn validation_errors_return_forms_to_the_correct_targets() {
        let pool = PgPoolOptions::new().connect_lazy("postgresql://localhost/unused").unwrap();
        pool.close().await;
        let state = AppState { pool: Arc::new(pool) };
        let result = model_create(State(state.clone()), Form(ModelForm::default())).await;
        let body = error_body(result, StatusCode::UNPROCESSABLE_ENTITY, "#modal", "outerHTML").await;
        assert!(body.contains("Name is required"));
        let result = model_update(State(state), Path(42), Form(ModelForm::default())).await;
        let body = error_body(result, StatusCode::UNPROCESSABLE_ENTITY, "#row-42", "outerHTML").await;
        assert!(body.starts_with("<tr id=\"row-42\">"));
        assert!(body.contains("Name is required"));
        assert!(body.contains("hx-put=\"/models/42\""));
    }

    #[tokio::test]
    async fn unavailable_database_returns_errors_instead_of_panicking_or_empty_lists() {
        let pool = PgPoolOptions::new().connect_lazy("postgresql://localhost/unused").unwrap();
        pool.close().await;
        let state = AppState { pool: Arc::new(pool) };
        let results = [
            models_list(State(state.clone()), HeaderMap::new(), Query(ListParams {
                q: None, min_score: None, sort: None,
            })).await,
            model_detail(State(state.clone()), Path(1)).await,
            model_edit_form(State(state.clone()), Path(1)).await,
            model_video_row(State(state.clone()), Path(1), Query(VideoParams { kind: None })).await,
            videos_list(State(state.clone())).await,
            model_create(State(state.clone()), Form(rejected_form())).await,
            model_update(State(state.clone()), Path(1), Form(rejected_form())).await,
            model_delete(State(state), Path(1)).await,
        ];
        for result in results {
            let response = result.unwrap_err().into_response();
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    #[sqlx::test]
    async fn missing_models_return_not_found_and_preserve_unsaved_edits(pool: PgPool) {
        let state = AppState { pool: Arc::new(pool) };
        let result = model_update(State(state.clone()), Path(999), Form(rejected_form())).await;
        let body = error_body(result, StatusCode::NOT_FOUND, "#row-999", "outerHTML").await;
        assert_entries_preserved(&body);
        for result in [
            model_detail(State(state.clone()), Path(999)).await,
            model_edit_form(State(state.clone()), Path(999)).await,
            model_video_row(State(state.clone()), Path(999), Query(VideoParams { kind: None })).await,
            model_delete(State(state), Path(999)).await,
        ] {
            error_body(result, StatusCode::NOT_FOUND, "#request-error", "innerHTML").await;
        }
    }

    async fn prepare_id_sequence(pool: &PgPool, existing_id: Option<i32>) {
        let mut tx = pool.begin().await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/0001_init.sql"))
            .execute(&mut *tx).await.unwrap();
        if let Some(id) = existing_id {
            sqlx::query("UPDATE models SET id = $1 WHERE id = 7")
                .bind(id).execute(&mut *tx).await.unwrap();
        } else {
            sqlx::query("DELETE FROM models")
                .execute(&mut *tx).await.unwrap();
        }
        sqlx::raw_sql(include_str!("../migrations/0003_models_id_sequence.sql"))
            .execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
    }

    #[sqlx::test(migrations = false)]
    async fn concurrent_creates_use_ids_above_existing_rows(pool: PgPool) {
        prepare_id_sequence(&pool, Some(1000)).await;
        let state = AppState { pool: Arc::new(pool) };
        let mut creates = tokio::task::JoinSet::new();
        for _ in 0..16 {
            let state = state.clone();
            creates.spawn(async move {
                model_create(State(state), Form(ModelForm {
                    // A submitted ID must not override the database default.
                    id: Some(1),
                    name: "generated".into(),
                    ..Default::default()
                })).await
            });
        }
        while let Some(result) = creates.join_next().await {
            let Html(row) = result.unwrap().unwrap();
            assert!(row.contains("<tr id=\"row-"));
        }
        let ids: Vec<i32> = sqlx::query_scalar(
            "SELECT id FROM models WHERE name = 'generated' ORDER BY id"
        ).fetch_all(&*state.pool).await.unwrap();
        assert_eq!(ids, (1001..=1016).collect::<Vec<_>>());
    }

    #[sqlx::test(migrations = false)]
    async fn create_in_empty_table_starts_at_one(pool: PgPool) {
        prepare_id_sequence(&pool, None).await;
        let state = AppState { pool: Arc::new(pool) };
        let Html(row) = model_create(State(state.clone()), Form(ModelForm {
            name: "first".into(),
            ..Default::default()
        })).await.unwrap();
        assert!(row.contains("<tr id=\"row-1\">"));
        let id: i32 = sqlx::query_scalar("SELECT id FROM models")
            .fetch_one(&*state.pool).await.unwrap();
        assert_eq!(id, 1);
    }

    #[test]
    fn video_file_name_examples() {
        assert_eq!(video_file_name("https://www.dropbox.com/scl/fi/abc/April01.mp4?rlkey=x&dl=0"), Some("April01.mp4"));
        assert_eq!(video_file_name("/static/video/April01.mp4"), Some("April01.mp4"));
        assert_eq!(video_file_name("https://example.com/page.html"), None);
    }
}
