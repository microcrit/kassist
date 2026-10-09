use licensing::{router_with_paypal_and_persistence, LicenseStore, PaypalVerifier};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let seed = LicenseStore::from_environment().unwrap_or_else(|error| {
        eprintln!("licensing: {error}");
        std::process::exit(2);
    });
    let store_path = std::env::var_os("KASSIST_STORE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./kassist-license-store.json"));
    let store = LicenseStore::load_or_seed(&store_path, seed).unwrap_or_else(|error| {
        eprintln!("licensing: {error}");
        std::process::exit(2);
    });
    let paypal = PaypalVerifier::from_environment().unwrap_or_else(|error| {
        eprintln!("licensing: {error}");
        std::process::exit(2);
    });
    let address = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:3001".into());
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| {
            eprintln!("licensing: cannot bind {address}: {error}");
            std::process::exit(2);
        });
    println!("licensing server listening on http://{address}");
    axum::serve(
        listener,
        router_with_paypal_and_persistence(store, paypal, store_path),
    )
    .await
    .unwrap_or_else(|error| {
        eprintln!("licensing: server stopped: {error}");
        std::process::exit(1);
    });
}
