use askama::Template;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Html,
    routing::get,
    Router,
};
use serde::Deserialize;
use std::sync::{Arc, RwLock};
use tower_http::{services::ServeDir, trace::TraceLayer};
use uuid::Uuid;

#[derive(Clone, Debug)]
struct AppState {
    products: Arc<RwLock<Vec<Product>>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ListParams {
    q: Option<String>,
    max_price: Option<f64>,
}

#[derive(Debug, Clone)]
struct Product {
    id: String,
    name: String,
    price: f64,
}

impl Product {
    fn demo() -> Vec<Product> {
        vec![
            Product { id: Uuid::new_v4().to_string(), name: "Victron 100/50 Fan Adapter".into(), price: 49.0 },
            Product { id: Uuid::new_v4().to_string(), name: "Raspberry Pi 5 Case".into(), price: 19.0 },
            Product { id: Uuid::new_v4().to_string(), name: "40mm Noctua Fan".into(), price: 14.5 },
            Product { id: Uuid::new_v4().to_string(), name: "ESP32 Dev Board".into(), price: 8.9 },
        ]
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTpl<'a> {
    title: &'a str,
}

#[derive(Template)]
#[template(path = "products.html")]
struct ProductsTpl<'a> {
    title: &'a str,
    products: &'a [Product],
    q_value: &'a str,
    max_price_value: &'a str,
}

#[derive(Template)]
#[template(path = "_rows.html")]
struct RowsTpl<'a> {
    products: &'a [Product],
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let state = AppState {
        products: Arc::new(RwLock::new(Product::demo())),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/products", get(products))
        .with_state(state)
        .nest_service("/static", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http());

    let addr = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("listening on http://{addr}");
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> Html<String> {
    Html(IndexTpl { title: "Home" }.render().unwrap())
}

async fn products(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListParams>,
) -> Html<String> {
    let items = state.products.read().unwrap().clone();
    let filtered: Vec<Product> = items
        .into_iter()
        .filter(|p| {
            let q_ok = params
                .q
                .as_ref()
                .map(|q| p.name.to_lowercase().contains(&q.to_lowercase()))
                .unwrap_or(true);
            let price_ok = params.max_price.map(|m| p.price <= m).unwrap_or(true);
            q_ok && price_ok
        })
        .collect();

    if headers.get("HX-Request").is_some() {
        return Html(RowsTpl { products: &filtered }.render().unwrap());
    }

    let q_val_owned = params.q.clone().unwrap_or_default();
    let max_val_owned = params
        .max_price
        .map(|m| m.to_string())
        .unwrap_or_default();

    Html(
        ProductsTpl {
            title: "Products",
            products: &filtered,
            q_value: &q_val_owned,
            max_price_value: &max_val_owned,
        }
            .render()
            .unwrap(),
    )
}
