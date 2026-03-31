mod db;
mod models;
mod routes;
mod storage;

use std::sync::Arc;
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Storage: S3/MinIO if configured, otherwise local
    let storage: Arc<dyn storage::Storage> = if let Ok(bucket) = std::env::var("S3_BUCKET") {
        #[cfg(feature = "s3")]
        {
            let endpoint = std::env::var("S3_ENDPOINT").expect("S3_ENDPOINT must be set");
            let region = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
            let url_prefix = std::env::var("S3_URL_PREFIX").expect("S3_URL_PREFIX must be set");
            Arc::new(storage::S3Storage::new(&bucket, &endpoint, &region, &url_prefix).await)
        }
        #[cfg(not(feature = "s3"))]
        {
            let _ = bucket;
            panic!("S3_BUCKET is set but s3 feature is not enabled. Build with: cargo build --features s3");
        }
    } else {
        let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "./uploads".to_string());
        let upload_url_prefix = std::env::var("UPLOAD_URL_PREFIX").unwrap_or_else(|_| "/uploads".to_string());
        Arc::new(storage::LocalStorage::new(&upload_dir, &upload_url_prefix))
    };

    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "./uploads".to_string());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .nest("/api", routes::api_router().with_state(pool.clone()))
        .nest("/api", routes::upload_router().with_state((pool.clone(), storage)))
        .nest("/ai", routes::ai_router().with_state(pool))
        // Serve local uploads (still useful as fallback)
        .nest_service("/uploads", ServeDir::new(&upload_dir))
        .layer(cors);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3002".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("Server running on http://localhost:{port}");
    axum::serve(listener, app).await.unwrap();
}
