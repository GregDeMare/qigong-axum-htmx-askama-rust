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
    #[sqlx(rename = "name_short")]
    pub nameShort: String,
    pub description: String,
    pub comments: String,
    #[sqlx(rename = "num_of_times")]
    pub numOfTimes: i32,
    #[sqlx(rename = "num_of_times_short")]
    pub numOfTimesShort: i32,
    pub score: i32,
    #[sqlx(rename = "score_short")]
    pub scoreShort: i32,
    pub imgname: String,
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
    #[serde(rename = "nameShort")]
    name_short: String,
    description: String,
    comments: String,
    #[serde(default)]
    numOfTimes: i32,
    #[serde(default)]
    numOfTimesShort: i32,
    #[serde(default)]
    score: i32,
    #[serde(default)]
    scoreShort: i32,
    imgname: String,
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
            imgname             TEXT NOT NULL
        );
    "#).execute(pool).await?;

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM models").fetch_one(pool).await?;
    if count == 0 {
        sqlx::query(r#"
            INSERT INTO models (id, name, name_short, description, comments, num_of_times, num_of_times_short, score, score_short, imgname)
            VALUES
              (1,'Morning Qigong','Qigong','15-min qigong routine','Great before breakfast',42,7,91,9,'qigong.png'),
              (2,'Judo Randori','Randori','Light sparring','Focus on footwork',23,4,78,8,'judo.png'),
              (3,'Meditation','Zen','10 min breathing','Evening wind-down',60,10,88,9,'zen.png')
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
        "SELECT id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,imgname FROM models"
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

    if headers.get("HX-Request").is_some() {
        return Html(RowsTpl { items: &items }.render().unwrap());
    }

    let q_value = params.q.unwrap_or_default();
    let min_score_value = params.min_score.map(|v| v.to_string()).unwrap_or_default();
    let sort_value = params.sort.unwrap_or_default();

    Html(ModelsTpl {
        title: "Models",
        items: &items,
        q_value: &q_value,
        min_score_value: &min_score_value,
        sort_value: &sort_value,
    }.render().unwrap())
}

// DETAIL
async fn model_detail(State(state): State<AppState>, Path(id): Path<i32>) -> Html<String> {
    let item = sqlx::query_as::<_, Model>(
        "SELECT id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,imgname FROM models WHERE id=$1"
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
        INSERT INTO models (id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,imgname)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
        RETURNING id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,imgname
    "#;

    let item: Model = sqlx::query_as(q)
        .bind(form.id.unwrap())
        .bind(&form.name)
        .bind(&form.name_short)
        .bind(&form.description)
        .bind(&form.comments)
        .bind(form.numOfTimes)
        .bind(form.numOfTimesShort)
        .bind(form.score)
        .bind(form.scoreShort)
        .bind(&form.imgname)
        .fetch_one(&*state.pool).await.unwrap();

    Html(RowTpl { m: &item }.render().unwrap())
}

// EDIT FORM (fragment)
async fn model_edit_form(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Html<String> {
    let item: Option<Model> = sqlx::query_as(
        "SELECT id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,imgname FROM models WHERE id=$1"
    ).bind(id).fetch_optional(&*state.pool).await.unwrap();

    if let Some(i) = item {
        let values = ModelForm {
            id: Some(i.id),
            name: i.name,
            name_short: i.nameShort,
            description: i.description,
            comments: i.comments,
            numOfTimes: i.numOfTimes,
            numOfTimesShort: i.numOfTimesShort,
            score: i.score,
            scoreShort: i.scoreShort,
            imgname: i.imgname,
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
          num_of_times=$5, num_of_times_short=$6, score=$7, score_short=$8, imgname=$9
        WHERE id=$10
        RETURNING id,name,name_short,description,comments,num_of_times,num_of_times_short,score,score_short,imgname
    "#;

    let item: Option<Model> = sqlx::query_as(q)
        .bind(&form.name)
        .bind(&form.name_short)
        .bind(&form.description)
        .bind(&form.comments)
        .bind(form.numOfTimes)
        .bind(form.numOfTimesShort)
        .bind(form.score)
        .bind(form.scoreShort)
        .bind(&form.imgname)
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
