use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
pub struct AppState {
    store: Arc<Mutex<LicenseStore>>,
    paypal: Option<PaypalVerifier>,
    persistence: Option<LicenseStoreFile>,
}
impl AppState {
    pub fn new(store: LicenseStore) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            paypal: None,
            persistence: None,
        }
    }
    pub fn with_paypal(store: LicenseStore, paypal: PaypalVerifier) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            paypal: Some(paypal),
            persistence: None,
        }
    }

    fn with_persistence(mut self, path: impl Into<PathBuf>) -> Self {
        self.persistence = Some(LicenseStoreFile { path: path.into() });
        self
    }

    fn persist(&self, store: &LicenseStore) -> Result<(), std::io::Error> {
        match &self.persistence {
            Some(file) => file.write(store),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LicenseStore {
    licenses: BTreeMap<String, Entitlement>,
    activations: BTreeMap<String, BTreeSet<String>>,
    payment_captures: BTreeSet<String>,
}
impl LicenseStore {
    pub fn empty() -> Self {
        Self {
            licenses: BTreeMap::new(),
            activations: BTreeMap::new(),
            payment_captures: BTreeSet::new(),
        }
    }
    pub fn from_environment() -> Result<Self, ConfigError> {
        Self::from_records(&std::env::var("KASSIST_LICENSES").unwrap_or_default())
    }

    pub fn load_or_seed(path: impl AsRef<Path>, seed: Self) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let mut store = if path.exists() {
            let contents = fs::read_to_string(path)
                .map_err(|error| ConfigError::Storage(error.to_string()))?;
            serde_json::from_str(&contents)
                .map_err(|error| ConfigError::Storage(error.to_string()))?
        } else {
            Self::empty()
        };
        store.licenses.extend(seed.licenses);
        Ok(store)
    }

    pub fn from_records(records: &str) -> Result<Self, ConfigError> {
        let mut store = Self::empty();
        for record in records
            .split(';')
            .filter(|record| !record.trim().is_empty())
        {
            let fields: Vec<_> = record.split('|').collect();
            if fields.len() != 4 || fields.iter().take(3).any(|field| field.trim().is_empty()) {
                return Err(ConfigError::InvalidRecord(record.into()));
            }
            let limit = fields[2]
                .parse::<usize>()
                .ok()
                .filter(|limit| *limit > 0)
                .ok_or_else(|| ConfigError::InvalidRecord(record.into()))?;
            let expires_at = if fields[3].trim().is_empty() {
                None
            } else {
                Some(
                    fields[3]
                        .parse()
                        .map_err(|_| ConfigError::InvalidRecord(record.into()))?,
                )
            };
            if store
                .licenses
                .insert(
                    fields[0].into(),
                    Entitlement {
                        product: fields[1].into(),
                        activation_limit: limit,
                        expires_at,
                    },
                )
                .is_some()
            {
                return Err(ConfigError::DuplicateKey(fields[0].into()));
            }
        }
        Ok(store)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Entitlement {
    product: String,
    activation_limit: usize,
    expires_at: Option<i64>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigError {
    InvalidRecord(String),
    DuplicateKey(String),
    MissingEnvironment(&'static str),
    Storage(String),
}
impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRecord(value) => write!(f, "invalid license record '{value}'"),
            Self::DuplicateKey(key) => write!(f, "duplicate license key '{key}'"),
            Self::MissingEnvironment(name) => {
                write!(f, "missing required environment variable {name}")
            }
            Self::Storage(message) => write!(f, "license store: {message}"),
        }
    }
}

#[derive(Clone)]
struct LicenseStoreFile {
    path: PathBuf,
}

impl LicenseStoreFile {
    fn write(&self, store: &LicenseStore) -> Result<(), std::io::Error> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_vec_pretty(store).map_err(std::io::Error::other)?;
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, serialized)?;
        fs::rename(temporary, &self.path)
    }
}
impl std::error::Error for ConfigError {}

pub fn router(store: LicenseStore) -> Router {
    router_with_state(AppState::new(store))
}
pub fn router_with_paypal(store: LicenseStore, paypal: PaypalVerifier) -> Router {
    router_with_state(AppState::with_paypal(store, paypal))
}
pub fn router_with_paypal_and_persistence(
    store: LicenseStore,
    paypal: PaypalVerifier,
    path: impl Into<PathBuf>,
) -> Router {
    router_with_state(AppState::with_paypal(store, paypal).with_persistence(path))
}
fn router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/activations", post(activate))
        .route("/v1/webhooks/paypal", post(paypal_webhook))
        .with_state(state)
}

#[derive(Deserialize)]
struct ActivationRequest {
    product: String,
    license_key: String,
    installation: String,
}
#[derive(Serialize)]
struct ActivationResponse {
    state: &'static str,
    expires_at: Option<i64>,
    already_active: bool,
}
async fn activate(
    State(state): State<AppState>,
    Json(request): Json<ActivationRequest>,
) -> Result<Json<ActivationResponse>, ApiError> {
    if [
        request.product.as_str(),
        request.license_key.as_str(),
        request.installation.as_str(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        return Err(ApiError::bad_request(
            "product, license_key, and installation are required",
        ));
    }
    let now = unix_time().map_err(|_| ApiError::internal())?;
    let mut store = state.store.lock().map_err(|_| ApiError::internal())?;
    let entitlement = store
        .licenses
        .get(&request.license_key)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_license",
                "license key is not recognized",
            )
        })?
        .clone();
    if entitlement.product != request.product {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "wrong_product",
            "license is not valid for this plugin",
        ));
    }
    if entitlement
        .expires_at
        .is_some_and(|expires_at| now >= expires_at)
    {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "expired",
            "license has expired",
        ));
    }
    let activations = store.activations.entry(request.license_key).or_default();
    let already_active = activations.contains(&request.installation);
    if !already_active && activations.len() >= entitlement.activation_limit {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "activation_limit",
            "activation limit has been reached",
        ));
    }
    activations.insert(request.installation);
    let snapshot = store.clone();
    drop(store);
    state.persist(&snapshot).map_err(|_| ApiError::internal())?;
    Ok(Json(ActivationResponse {
        state: "active",
        expires_at: entitlement.expires_at,
        already_active,
    }))
}

#[derive(Clone)]
pub struct PaypalVerifier {
    client_id: String,
    secret: String,
    webhook_id: String,
    api_base: String,
    client: reqwest::Client,
}
impl PaypalVerifier {
    pub fn from_environment() -> Result<Self, ConfigError> {
        let get = |name| std::env::var(name).map_err(|_| ConfigError::MissingEnvironment(name));
        Ok(Self {
            client_id: get("PAYPAL_CLIENT_ID")?,
            secret: get("PAYPAL_CLIENT_SECRET")?,
            webhook_id: get("PAYPAL_WEBHOOK_ID")?,
            api_base: std::env::var("PAYPAL_API_BASE")
                .unwrap_or_else(|_| "https://api-m.paypal.com".into())
                .trim_end_matches('/')
                .into(),
            client: reqwest::Client::new(),
        })
    }
    async fn verify(
        &self,
        headers: &HeaderMap,
        event: serde_json::Value,
    ) -> Result<(), PaypalError> {
        let token: PaypalToken = self
            .client
            .post(format!("{}/v1/oauth2/token", self.api_base))
            .basic_auth(&self.client_id, Some(&self.secret))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
            .map_err(PaypalError::transport)?
            .error_for_status()
            .map_err(PaypalError::transport)?
            .json()
            .await
            .map_err(PaypalError::transport)?;
        let response: PaypalVerification = self
            .client
            .post(format!(
                "{}/v1/notifications/verify-webhook-signature",
                self.api_base
            ))
            .bearer_auth(token.access_token)
            .json(&VerifyRequest {
                auth_algo: header(headers, "paypal-auth-algo")?,
                cert_url: header(headers, "paypal-cert-url")?,
                transmission_id: header(headers, "paypal-transmission-id")?,
                transmission_sig: header(headers, "paypal-transmission-sig")?,
                transmission_time: header(headers, "paypal-transmission-time")?,
                webhook_id: self.webhook_id.clone(),
                webhook_event: event,
            })
            .send()
            .await
            .map_err(PaypalError::transport)?
            .error_for_status()
            .map_err(PaypalError::transport)?
            .json()
            .await
            .map_err(PaypalError::transport)?;
        if response.verification_status == "SUCCESS" {
            Ok(())
        } else {
            Err(PaypalError::InvalidSignature)
        }
    }
}
#[derive(Serialize)]
struct VerifyRequest {
    auth_algo: String,
    cert_url: String,
    transmission_id: String,
    transmission_sig: String,
    transmission_time: String,
    webhook_id: String,
    webhook_event: serde_json::Value,
}
#[derive(Deserialize)]
struct PaypalToken {
    access_token: String,
}
#[derive(Deserialize)]
struct PaypalVerification {
    verification_status: String,
}
enum PaypalError {
    MissingHeader,
    InvalidSignature,
    Transport(String),
}
impl PaypalError {
    fn transport(error: reqwest::Error) -> Self {
        Self::Transport(error.to_string())
    }
}
fn header(headers: &HeaderMap, name: &'static str) -> Result<String, PaypalError> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .ok_or(PaypalError::MissingHeader)
}

async fn paypal_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(event): Json<serde_json::Value>,
) -> Result<StatusCode, ApiError> {
    let verifier = state.paypal.as_ref().ok_or_else(ApiError::unavailable)?;
    verifier
        .verify(&headers, event.clone())
        .await
        .map_err(|error| match error {
            PaypalError::MissingHeader | PaypalError::InvalidSignature => ApiError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_paypal_webhook",
                "PayPal signature verification failed",
            ),
            PaypalError::Transport(message) => {
                let _ = message;
                ApiError::unavailable()
            }
        })?;
    if event.get("event_type").and_then(serde_json::Value::as_str)
        != Some("PAYMENT.CAPTURE.COMPLETED")
    {
        return Ok(StatusCode::NO_CONTENT);
    }
    let capture = event
        .get("resource")
        .and_then(|resource| resource.get("id"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("completed PayPal event has no capture ID"))?;
    let mut store = state.store.lock().map_err(|_| ApiError::internal())?;
    store.payment_captures.insert(capture.into());
    let snapshot = store.clone();
    drop(store);
    state.persist(&snapshot).map_err(|_| ApiError::internal())?;
    Ok(StatusCode::ACCEPTED)
}

fn unix_time() -> Result<i64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}
impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            code,
            message,
        }
    }
    fn bad_request(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }
    fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal server error",
        )
    }
    fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "payment verification is unavailable",
        )
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.code, "message": self.message })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::{router, router_with_state, AppState, LicenseStore};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    #[tokio::test]
    async fn activation_is_idempotent() {
        let app = router(LicenseStore::from_records("key|cc.hellings.simassist|1|").unwrap());
        let request = || {
            Request::builder().method("POST").uri("/v1/activations").header("content-type", "application/json").body(Body::from(r#"{"product":"cc.hellings.simassist","license_key":"key","installation":"machine"}"#)).unwrap()
        };
        assert_eq!(
            app.clone().oneshot(request()).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            app.oneshot(request()).await.unwrap().status(),
            StatusCode::OK
        );
    }
    #[test]
    fn rejects_bad_license_records() {
        assert!(LicenseStore::from_records("key|plugin|0|").is_err());
    }

    #[tokio::test]
    async fn activation_history_is_persisted() {
        let path = std::env::temp_dir().join(format!(
            "kassist-licensing-test-{}-{}.json",
            std::process::id(),
            super::unix_time().unwrap()
        ));
        let store = LicenseStore::from_records("key|cc.hellings.simassist|1|").unwrap();
        let app = router_with_state(AppState::new(store).with_persistence(&path));
        let request = Request::builder()
            .method("POST")
            .uri("/v1/activations")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"product":"cc.hellings.simassist","license_key":"key","installation":"machine"}"#))
            .unwrap();
        assert_eq!(app.oneshot(request).await.unwrap().status(), StatusCode::OK);
        let loaded = LicenseStore::load_or_seed(&path, LicenseStore::empty()).unwrap();
        assert!(loaded
            .activations
            .get("key")
            .is_some_and(|machines| machines.contains("machine")));
        std::fs::remove_file(path).unwrap();
    }
}
