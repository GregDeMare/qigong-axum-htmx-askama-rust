use askama::Template;
use axum::{
    extract::{Form, Path, Query, State},
    http::HeaderMap,
    response::Html,
    routing::{delete, get, post, put},
    Router,
};
use dotenvy::dotenv;
use serde::Deserialize;
use sqlx::{FromRow, PgPool, postgres::PgPoolOptions, QueryBuilder};
use std::sync::Arc;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::{error, info};

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

#[derive(Clone)]
struct AppState {
    pool: Arc<PgPool>,
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
#[template(path = "index.html")]
struct IndexTpl<'a> { title: &'a str }

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
    dotenv().ok();
    tracing_subscriber::fmt().with_env_filter("info").init();

    let db_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new().max_connections(10).connect(&db_url).await?;
    bootstrap(&pool).await?;

    let state = AppState { pool: Arc::new(pool) };

    let app = Router::new()
        // pages
        .route("/", get(index))
        .route("/models", get(models_list).post(model_create))
        .route("/models/new", get(model_new_form))
        .route("/models/:id", get(model_detail).put(model_update).delete(model_delete))
        .route("/models/:id/edit", get(model_edit_form))
        .route("/models/:id/video", get(model_video_row))
        // static
        .with_state(state)
        .nest_service("/static", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http());

    let addr = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
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
async fn bootstrap(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS models (
            id                  INTEGER PRIMARY KEY,
            name                TEXT NOT NULL,
            name_short          TEXT NOT NULL,
            description         TEXT NOT NULL,
            comments            TEXT NOT NULL,
            num_of_times        INTEGER NOT NULL,
            num_of_times_short  INTEGER NOT NULL,
            score               INTEGER NOT NULL,
            score_short         INTEGER NOT NULL,
            image_name          TEXT NOT NULL,
            long_url            TEXT NOT NULL,
            short_url           TEXT NOT NULL
        );
    "#).execute(pool).await?;

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM models").fetch_one(pool).await?;
    if count == 0 {
        sqlx::query(r#"
            INSERT INTO models (id, name, name_short, description, comments, num_of_times, num_of_times_short, score, score_short, image_name, long_url, short_url)
            VALUES
              (1,'April01','April01Short','Side to side look at hands','',2,1,4,4,'April01.png', 'https://www.dropbox.com/scl/fi/jbp8cca0t9lmm1wh8pewx/April01.mp4?rlkey=rdwxc13kbmk2oihd1tb0i1ci1&dl=0', 'https://www.dropbox.com/scl/fi/me6tdolc7mt4zr7s4f4p1/April01Short.mp4?rlkey=2dfkcf10t3qfihfmrfclp5mqs&dl=0'),
              (2,'April04','April04Short','Flows focused on the White Crane','',3,4,4,3,'April04.png', 'https://www.dropbox.com/scl/fi/ppqw7r4amekpnxk3t5ouy/April04.mp4?rlkey=cz04vy5u0rwce9hg8nvtlk3p1&dl=0', 'https://www.dropbox.com/scl/fi/1nkv3brh8ruj3c87clfwt/April04Short.mp4?rlkey=97car8u968b1w0xkbyg7uirxi&dl=0'),
              (3,'April06','April06Short','Flows focused on Cloudy Hands and Centering','',2,1,4,4,'April06.png', 'https://www.dropbox.com/scl/fi/k3fd7ymnbxqw23v66cboj/April06.mp4?rlkey=xxviixzg4z4w9gs7gmf5jyrvi&dl=0', 'https://www.dropbox.com/scl/fi/qh2ygprv6dyvr8mbczoz2/April06Short.mp4?rlkey=cjv0he2vaw4e2kxjuoje7wehx&dl=0'),
              (4,'April08','April08Short','Bone breathing and flying in the flows','',3,4,4,3,'April08.png', 'https://www.dropbox.com/scl/fi/3cphx2fczf4mhsbgo7zk2/April08.mp4?rlkey=9wpd1szgbe322dpynzabjkm7n&dl=0', 'https://www.dropbox.com/scl/fi/ph4q5v4109mfbusuvu40a/April08Short.mp4?rlkey=y557s7mzejrbt3mqto3trygsv&dl=0'),
              (5,'April11','April11Short','Dragon Creates the Pearl','',3,4,4,3,'April11.png', 'https://www.dropbox.com/scl/fi/sj6fjdlk5u9tuo0jzxya1/April11.mp4?rlkey=rq9znrbe82tfal1e4rruo7394&dl=0', 'https://www.dropbox.com/scl/fi/89wbsznf58yluvcfjqags/April11Short.mp4?rlkey=tq6o2rzl3wnqlvso6jutevmlx&dl=0'),
              (6,'April13','April13Short','Flow focused on the earth element','',3,4,4,3,'April15.png', 'https://www.dropbox.com/scl/fi/bspzmtbfdn1kblt7ezne1/April13.mp4?rlkey=5foj1vemlvljimzjf4klorhn7&dl=0', 'https://www.dropbox.com/scl/fi/ws9p41n393evwi4kxyibw/April13Short.mp4?rlkey=nriwexi5r6fmz1cvg36m9dhea&dl=0'),
              (7,'April15','April15Short','Class focused on calming the mind','',3,4,4,3,'April15.png', 'https://www.dropbox.com/scl/fi/9fbrframbv56ylipko640/April15.mp4?rlkey=m1q3l4zgdk5n95r9eevc4rf65&dl=0', 'https://www.dropbox.com/scl/fi/ytmzcl1st2u3c2f1rc9tm/April15Short.mp4?rlkey=opzkmm021uuycend2jihaorao&dl=0')
            ON CONFLICT (id) DO NOTHING;
        "#).execute(pool).await?;
    }
    Ok(())
}

async fn index() -> Html<String> {
    Html(IndexTpl { title: "Home" }.render().unwrap())
}

// LIST + FILTER
async fn models_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListParams>,
) -> Html<String> {
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

    let items: Vec<Model> = qb.build_query_as().fetch_all(&*state.pool).await.unwrap_or_else(|e| {
        error!("query failed: {e}");
        vec![]
    });

    let is_hx = headers.get("HX-Request").is_some();
    let hx_target = headers
        .get("HX-Target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Only send the fragment when the request is explicitly targeting the table body
    if is_hx && hx_target == "rows" {
        return Html(RowsTpl { items: &items }.render().unwrap());
    }

    // otherwise, render full page (so headers appear)
    let q_value = params.q.unwrap_or_default();
    let min_score_value = params.min_score.map(|v| v.to_string()).unwrap_or_default();
    let sort_value = params.sort.unwrap_or_default();

    Html(ModelsTpl {
        title: "Qigong Videos",
        items: &items,
        q_value: &q_value,
        min_score_value: &min_score_value,
        sort_value: &sort_value,
    }
        .render()
        .unwrap())
}
// DETAIL
async fn model_detail(State(state): State<AppState>, Path(id): Path<i32>) -> Html<String> {
    let item = sqlx::query_as::<_, Model>(
        "SELECT id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name, long_url, short_url FROM models WHERE id=$1"
    ).bind(id).fetch_optional(&*state.pool).await.unwrap();

    match item {
        Some(item) => Html(DetailTpl { title: "Model Detail", item: &item }.render().unwrap()),
        None => Html("<h1>Not found</h1>".into()),
    }
}

// NEW FORM (fragment)
async fn model_new_form() -> Html<String> {
    let empty = ModelForm::default();
    Html(FormTpl {
        action: "/models",
        method: "post",
        submit_label: "Create",
        target_id: "rows",          // ✅ where to insert the new <tr>
        swap_mode: "beforeend",     // ✅ append to #rows
        values: &empty,
        error: "",
    }.render().unwrap())
}

// CREATE (returns a single <tr> to append or errors as form)
async fn model_create(
    State(state): State<AppState>,
    Form(mut form): Form<ModelForm>,
) -> Html<String> {
    // naive validation
    if form.name.trim().is_empty() {
        return Html(FormTpl {
            action: "/models",
            method: "post",
            submit_label: "Create",
            target_id: "rows",          // ✅
            swap_mode: "beforeend",     // ✅
            values: &form,
            error: "Name is required",
        }.render().unwrap());
    }

    // if no id provided, auto-generate next id
    if form.id.is_none() {
        let (next_id,): (i64,) = sqlx::query_as("SELECT COALESCE(MAX(id)+1,1) FROM models")
            .fetch_one(&*state.pool).await.unwrap();
        form.id = Some(next_id as i32);
    }

    let q = r#"
        INSERT INTO models (id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name, long_url, short_url)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
        RETURNING id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name, long_url, short_url
    "#;

    let item: Model = sqlx::query_as(q)
        .bind(form.id.unwrap())
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
        .fetch_one(&*state.pool).await.unwrap();

    Html(RowTpl { m: &item }.render().unwrap())
}

// EDIT FORM (fragment)
async fn model_edit_form(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Html<String> {
    let item: Option<Model> = sqlx::query_as(
        "SELECT id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name, long_url, short_url FROM models WHERE id=$1"
    ).bind(id).fetch_optional(&*state.pool).await.unwrap();

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
        Html(FormRowTpl { values: &values, error: "" }.render().unwrap())
    } else {
        Html("<tr><td colspan=\"8\">Not found</td></tr>".into())
    }
}

// UPDATE (returns the updated <tr>)
async fn model_update(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Form(form): Form<ModelForm>,
) -> Html<String> {
    if form.name.trim().is_empty() {
        let action_owned = format!("/models/{id}");
        let target_owned = format!("row-{id}");
        return Html(FormTpl {
            action: Box::leak(action_owned.into_boxed_str()),
            method: "put",
            submit_label: "Save",
            target_id: Box::leak(target_owned.into_boxed_str()), // ✅
            swap_mode: "outerHTML",                               // ✅
            values: &form,
            error: "Name is required",
        }.render().unwrap());
    }
    let q = r#"
        UPDATE models SET
          name=$1, name_short=$2, description=$3, comments=$4,
          num_of_times=$5, num_of_times_short=$6, score=$7, score_short=$8, image_name=$9, long_url=$10, short_url=$11
        WHERE id=$10
        RETURNING id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,image_name,long_url,short_url
    "#;

    let item: Option<Model> = sqlx::query_as(q)
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
        .fetch_optional(&*state.pool).await.unwrap();

    match item {
        Some(i) => Html(RowTpl { m: &i }.render().unwrap()),
        None => Html("<tr><td colspan=\"7\">Not found</td></tr>".into()),
    }
}

// DELETE (returns an empty string; htmx will remove the row)
async fn model_delete(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Html<String> {
    let _ = sqlx::query("DELETE FROM models WHERE id=$1").bind(id).execute(&*state.pool).await;
    Html(String::new())
}

async fn model_video_row(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Query(params): Query<VideoParams>,
) -> Html<String> {
    let item = sqlx::query_as::<_, Model>(
        "SELECT id,name,name_short,description,comments,
                num_of_times,num_of_times_short,score,score_short,image_name,long_url,short_url
         FROM models WHERE id=$1"
    )
        .bind(id)
        .fetch_optional(&*state.pool)
        .await
        .unwrap();

    if let Some(m) = item {
        let use_short = matches!(params.kind.as_deref(), Some("short"));
        let src = if use_short { m.short_url.clone() } else { m.long_url.clone() };
        // ensure normalized before feeding to the template (in case legacy rows exist)
        let src = normalize_dropbox_url(&src).unwrap_or(src);

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

        return Html(VideoRowTpl {
            id: m.id,
            poster,
            src,
            title,
            colspan: 8,
        }.render().unwrap());
    }

    Html("<tr><td colspan=\"8\">Not found</td></tr>".into())
}

