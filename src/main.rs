use risk_compiler_a::build_app_spawned;

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:3001";

    let (app, _st) = build_app_spawned().await;

    println!("listen http://{addr}\n  curl -s http://{addr}/ | jq .");
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("failed to bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("shutdown");
    };
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
    {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}
