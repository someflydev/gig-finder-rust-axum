//! Immutable artifact storage + HTTP fetch utilities for RHOF.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, Semaphore};
use tracing::info_span;
use uuid::Uuid;

pub const CRATE_NAME: &str = "rhof-storage";

#[derive(Debug, Clone)]
pub struct StoredArtifact {
    pub content_hash: String,
    pub relative_path: PathBuf,
    pub absolute_path: PathBuf,
    pub byte_size: usize,
    pub deduplicated: bool,
}

#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    pub fn artifact_relative_path(
        &self,
        fetched_at: DateTime<Utc>,
        source_id: &str,
        content_hash: &str,
        extension: &str,
    ) -> PathBuf {
        let stamp = fetched_at.format("%Y%m%d_%H%M%S").to_string();
        let ext = extension.trim_start_matches('.').trim();
        let ext = if ext.is_empty() { "bin" } else { ext };
        PathBuf::from(stamp)
            .join(source_id)
            .join(format!("{content_hash}.{ext}"))
    }

    /// Store bytes immutably using a hash-addressed path and atomic temp-file rename.
    pub async fn store_bytes(
        &self,
        fetched_at: DateTime<Utc>,
        source_id: &str,
        extension: &str,
        bytes: &[u8],
    ) -> anyhow::Result<StoredArtifact> {
        let content_hash = Self::sha256_hex(bytes);
        let relative_path =
            self.artifact_relative_path(fetched_at, source_id, &content_hash, extension);
        let absolute_path = self.root.join(&relative_path);

        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("creating artifact directory {}", parent.display()))?;
        }

        if fs::try_exists(&absolute_path)
            .await
            .with_context(|| format!("checking artifact path {}", absolute_path.display()))?
        {
            return Ok(StoredArtifact {
                content_hash,
                relative_path,
                absolute_path,
                byte_size: bytes.len(),
                deduplicated: true,
            });
        }

        let temp_name = format!(".{}.{}.tmp", Uuid::new_v4(), bytes.len());
        let temp_path = absolute_path
            .parent()
            .expect("artifact path always has parent")
            .join(temp_name);

        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .await
            .with_context(|| format!("opening temp artifact file {}", temp_path.display()))?;
        file.write_all(bytes)
            .await
            .with_context(|| format!("writing temp artifact file {}", temp_path.display()))?;
        file.flush()
            .await
            .with_context(|| format!("flushing temp artifact file {}", temp_path.display()))?;
        drop(file);

        match fs::rename(&temp_path, &absolute_path).await {
            Ok(()) => Ok(StoredArtifact {
                content_hash,
                relative_path,
                absolute_path,
                byte_size: bytes.len(),
                deduplicated: false,
            }),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temp_path).await;
                Ok(StoredArtifact {
                    content_hash,
                    relative_path,
                    absolute_path,
                    byte_size: bytes.len(),
                    deduplicated: true,
                })
            }
            Err(err) => {
                let _ = fs::remove_file(&temp_path).await;
                Err(err).with_context(|| {
                    format!(
                        "atomically renaming temp artifact {} -> {}",
                        temp_path.display(),
                        absolute_path.display()
                    )
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDisposition {
    Retryable,
    NonRetryable,
}

pub fn classify_status(status: StatusCode) -> RetryDisposition {
    if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        RetryDisposition::Retryable
    } else {
        RetryDisposition::NonRetryable
    }
}

pub fn classify_reqwest_error(err: &reqwest::Error) -> RetryDisposition {
    if err.is_timeout() || err.is_connect() || err.is_request() {
        RetryDisposition::Retryable
    } else {
        RetryDisposition::NonRetryable
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BackoffPolicy {
    pub max_retries: usize,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(5),
        }
    }
}

impl BackoffPolicy {
    pub fn delay_for_attempt(&self, attempt_index: usize) -> Duration {
        let factor = 1u32.checked_shl(attempt_index as u32).unwrap_or(u32::MAX);
        let delay = self.base_delay.saturating_mul(factor);
        delay.min(self.max_delay)
    }
}

#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub timeout: Duration,
    pub user_agent: Option<String>,
    pub global_concurrency: usize,
    pub per_source_concurrency: usize,
    pub backoff: BackoffPolicy,
    pub token_bucket: Option<TokenBucketConfig>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
            user_agent: None,
            global_concurrency: 16,
            per_source_concurrency: 4,
            backoff: BackoffPolicy::default(),
            token_bucket: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TokenBucketConfig {
    pub capacity: u32,
    pub refill_every: Duration,
}

#[derive(Debug)]
pub struct SimpleTokenBucket {
    capacity: u32,
    refill_every: Duration,
    state: Mutex<TokenBucketState>,
}

#[derive(Debug, Clone, Copy)]
struct TokenBucketState {
    tokens: u32,
    last_refill: Instant,
}

impl SimpleTokenBucket {
    pub fn new(capacity: u32, refill_every: Duration) -> Self {
        Self {
            capacity,
            refill_every,
            state: Mutex::new(TokenBucketState {
                tokens: capacity,
                last_refill: Instant::now(),
            }),
        }
    }

    pub async fn take(&self) {
        loop {
            let mut state = self.state.lock().await;
            let elapsed = state.last_refill.elapsed();
            if elapsed >= self.refill_every && self.refill_every.as_millis() > 0 {
                let refills = (elapsed.as_millis() / self.refill_every.as_millis()) as u32;
                state.tokens = (state.tokens.saturating_add(refills)).min(self.capacity);
                state.last_refill = Instant::now();
            }

            if state.tokens > 0 {
                state.tokens -= 1;
                return;
            }

            let sleep_for = self.refill_every;
            drop(state);
            tokio::time::sleep(sleep_for).await;
        }
    }
}

#[derive(Debug)]
pub struct HttpFetcher {
    client: reqwest::Client,
    global_limit: Arc<Semaphore>,
    per_source_limit: usize,
    per_source: Mutex<HashMap<String, Arc<Semaphore>>>,
    token_bucket: Option<Arc<SimpleTokenBucket>>,
    backoff: BackoffPolicy,
}

#[derive(Debug, Clone)]
pub struct FetchedResponse {
    pub status: StatusCode,
    pub final_url: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("request failed after retries: {0}")]
    Request(#[from] reqwest::Error),
    #[error("http status {status} for {url}")]
    HttpStatus { status: u16, url: String },
}

impl HttpFetcher {
    pub fn new(config: HttpClientConfig) -> anyhow::Result<Self> {
        let mut builder = reqwest::Client::builder()
            .gzip(true)
            .brotli(true)
            .timeout(config.timeout);

        if let Some(user_agent) = &config.user_agent {
            builder = builder.user_agent(user_agent.clone());
        }

        let client = builder.build().context("building reqwest client")?;
        let token_bucket = config
            .token_bucket
            .map(|c| Arc::new(SimpleTokenBucket::new(c.capacity, c.refill_every)));

        Ok(Self {
            client,
            global_limit: Arc::new(Semaphore::new(config.global_concurrency.max(1))),
            per_source_limit: config.per_source_concurrency.max(1),
            per_source: Mutex::new(HashMap::new()),
            token_bucket,
            backoff: config.backoff,
        })
    }

    async fn per_source_semaphore(&self, source_id: &str) -> Arc<Semaphore> {
        let mut map = self.per_source.lock().await;
        map.entry(source_id.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(self.per_source_limit)))
            .clone()
    }

    pub async fn fetch_bytes(
        &self,
        run_id: Uuid,
        source_id: &str,
        url: &str,
    ) -> Result<FetchedResponse, FetchError> {
        let _global = self.global_limit.acquire().await.expect("semaphore not closed");
        let per_source = self.per_source_semaphore(source_id).await;
        let _source = per_source.acquire().await.expect("semaphore not closed");

        if let Some(bucket) = &self.token_bucket {
            bucket.take().await;
        }

        let span = info_span!("http_fetch", %run_id, source_id, url);
        let _guard = span.enter();

        let mut last_request_error: Option<reqwest::Error> = None;

        for attempt in 0..=self.backoff.max_retries {
            let resp_result = self.client.get(url).send().await;

            match resp_result {
                Ok(resp) => {
                    let status = resp.status();
                    let final_url = resp.url().to_string();

                    if status.is_success() {
                        let body = resp.bytes().await?.to_vec();
                        return Ok(FetchedResponse {
                            status,
                            final_url,
                            body,
                        });
                    }

                    let disposition = classify_status(status);
                    if disposition == RetryDisposition::Retryable && attempt < self.backoff.max_retries
                    {
                        tokio::time::sleep(self.backoff.delay_for_attempt(attempt)).await;
                        continue;
                    }

                    return Err(FetchError::HttpStatus {
                        status: status.as_u16(),
                        url: final_url,
                    });
                }
                Err(err) => {
                    let disposition = classify_reqwest_error(&err);
                    if disposition == RetryDisposition::Retryable && attempt < self.backoff.max_retries
                    {
                        last_request_error = Some(err);
                        tokio::time::sleep(self.backoff.delay_for_attempt(attempt)).await;
                        continue;
                    }
                    return Err(FetchError::Request(err));
                }
            }
        }

        Err(FetchError::Request(
            last_request_error.expect("retry loop should capture a request error"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn artifact_hashing_is_stable() {
        let hash = ArtifactStore::sha256_hex(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[tokio::test]
    async fn atomic_writes_deduplicate_by_hash_path() {
        let dir = tempdir().expect("tempdir");
        let store = ArtifactStore::new(dir.path());
        let fetched_at = DateTime::parse_from_rfc3339("2026-02-24T12:00:00Z")
            .expect("ts")
            .with_timezone(&Utc);

        let first = store
            .store_bytes(fetched_at, "clickworker", "html", b"<html>same</html>")
            .await
            .expect("first store");
        let second = store
            .store_bytes(fetched_at, "clickworker", "html", b"<html>same</html>")
            .await
            .expect("second store");

        assert!(!first.deduplicated);
        assert!(second.deduplicated);
        assert_eq!(first.content_hash, second.content_hash);
        assert_eq!(first.relative_path, second.relative_path);
        assert!(first.absolute_path.exists());
    }

    #[test]
    fn backoff_logic_is_exponential_and_capped() {
        let policy = BackoffPolicy {
            max_retries: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(350),
        };

        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(350));
        assert_eq!(policy.delay_for_attempt(5), Duration::from_millis(350));
    }
}
