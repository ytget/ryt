//! Retry logic for downloads

use crate::error::RytError;
use std::time::Duration;

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}

/// Retry executor
pub struct RetryExecutor {
    config: RetryConfig,
}

impl RetryExecutor {
    /// Create a new retry executor
    pub fn new() -> Self {
        Self::with_config(RetryConfig::default())
    }

    /// Create a new retry executor with configuration
    pub fn with_config(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Execute a function with retry logic
    pub async fn execute<F, T>(&self, mut func: F) -> Result<T, RytError>
    where
        F: FnMut() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T, RytError>> + Send>,
        >,
    {
        let mut last_error = None;
        let mut delay = self.config.initial_delay;

        for attempt in 0..=self.config.max_retries {
            match func().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    last_error = Some(error);

                    // Check if error is retryable
                    if !last_error.as_ref().unwrap().is_retryable() {
                        break;
                    }

                    // Don't wait after the last attempt
                    if attempt < self.config.max_retries {
                        // Add jitter to prevent thundering herd
                        let jitter = if self.config.jitter_factor > 0.0 {
                            let jitter_range = delay.as_millis() as f64 * self.config.jitter_factor;
                            let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;
                            Duration::from_millis(jitter.abs() as u64)
                        } else {
                            Duration::from_millis(0)
                        };

                        let total_delay = delay + jitter;
                        tokio::time::sleep(total_delay).await;

                        // Calculate next delay with exponential backoff
                        delay = Duration::from_millis(
                            (delay.as_millis() as f64 * self.config.backoff_multiplier) as u64,
                        );

                        // Cap at maximum delay
                        if delay > self.config.max_delay {
                            delay = self.config.max_delay;
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or(RytError::Generic("All retry attempts failed".to_string())))
    }

    /// Execute a function with retry logic and custom error handling
    pub async fn execute_with_error_handler<F, T, E>(
        &self,
        mut func: F,
        error_handler: E,
    ) -> Result<T, RytError>
    where
        F: FnMut() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T, RytError>> + Send>,
        >,
        E: Fn(&RytError) -> bool, // Returns true if error is retryable
    {
        let mut last_error = None;
        let mut delay = self.config.initial_delay;

        for attempt in 0..=self.config.max_retries {
            match func().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    last_error = Some(error);

                    // Check if error is retryable using custom handler
                    if !error_handler(last_error.as_ref().unwrap()) {
                        break;
                    }

                    // Don't wait after the last attempt
                    if attempt < self.config.max_retries {
                        // Add jitter to prevent thundering herd
                        let jitter = if self.config.jitter_factor > 0.0 {
                            let jitter_range = delay.as_millis() as f64 * self.config.jitter_factor;
                            let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;
                            Duration::from_millis(jitter.abs() as u64)
                        } else {
                            Duration::from_millis(0)
                        };

                        let total_delay = delay + jitter;
                        tokio::time::sleep(total_delay).await;

                        // Calculate next delay with exponential backoff
                        delay = Duration::from_millis(
                            (delay.as_millis() as f64 * self.config.backoff_multiplier) as u64,
                        );

                        // Cap at maximum delay
                        if delay > self.config.max_delay {
                            delay = self.config.max_delay;
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or(RytError::Generic("All retry attempts failed".to_string())))
    }
}

impl Default for RetryExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Retry configuration builder
pub struct RetryConfigBuilder {
    config: RetryConfig,
}

impl RetryConfigBuilder {
    /// Create a new retry configuration builder
    pub fn new() -> Self {
        Self {
            config: RetryConfig::default(),
        }
    }

    /// Set maximum retries
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    /// Set initial delay
    pub fn initial_delay(mut self, initial_delay: Duration) -> Self {
        self.config.initial_delay = initial_delay;
        self
    }

    /// Set maximum delay
    pub fn max_delay(mut self, max_delay: Duration) -> Self {
        self.config.max_delay = max_delay;
        self
    }

    /// Set backoff multiplier
    pub fn backoff_multiplier(mut self, backoff_multiplier: f64) -> Self {
        self.config.backoff_multiplier = backoff_multiplier;
        self
    }

    /// Set jitter factor
    pub fn jitter_factor(mut self, jitter_factor: f64) -> Self {
        self.config.jitter_factor = jitter_factor.clamp(0.0, 1.0);
        self
    }

    /// Build the retry configuration
    pub fn build(self) -> RetryConfig {
        self.config
    }
}

impl Default for RetryConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(200));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.jitter_factor, 0.1);
    }

    #[test]
    fn test_retry_config_builder() {
        let config = RetryConfigBuilder::new()
            .max_retries(5)
            .initial_delay(Duration::from_millis(100))
            .max_delay(Duration::from_secs(60))
            .backoff_multiplier(1.5)
            .jitter_factor(0.2)
            .build();

        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_delay, Duration::from_millis(100));
        assert_eq!(config.max_delay, Duration::from_secs(60));
        assert_eq!(config.backoff_multiplier, 1.5);
        assert_eq!(config.jitter_factor, 0.2);
    }

    #[tokio::test]
    async fn test_retry_executor_success() {
        let executor = RetryExecutor::new();
        let counter = Arc::new(AtomicU32::new(0));

        let result: Result<String, RytError> = executor
            .execute({
                let counter = counter.clone();
                move || {
                    let counter = counter.clone();
                    Box::pin(async move {
                        let count = counter.fetch_add(1, Ordering::SeqCst);
                        if count == 0 {
                            Err(RytError::TimeoutError("test error".to_string()))
                        } else {
                            Ok("Success".to_string())
                        }
                    })
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_executor_max_retries() {
        let executor = RetryExecutor::new();
        let counter = Arc::new(AtomicU32::new(0));

        let result: Result<String, RytError> = executor
            .execute({
                let counter = counter.clone();
                move || {
                    let counter = counter.clone();
                    Box::pin(async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Err(RytError::TimeoutError("test error".to_string()))
                    })
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 4); // 1 initial + 3 retries
    }

    #[tokio::test]
    async fn test_retry_executor_non_retryable_error() {
        let executor = RetryExecutor::new();
        let counter = Arc::new(AtomicU32::new(0));

        let result: Result<String, RytError> = executor
            .execute({
                let counter = counter.clone();
                move || {
                    let counter = counter.clone();
                    Box::pin(async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Err(RytError::VideoUnavailable) // Non-retryable error
                    })
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Only one attempt
    }

    #[tokio::test]
    async fn test_retry_executor_with_custom_error_handler() {
        let executor = RetryExecutor::new();
        let counter = Arc::new(AtomicU32::new(0));

        let result: Result<String, RytError> = executor
            .execute_with_error_handler(
                {
                    let counter = counter.clone();
                    move || {
                        let counter = counter.clone();
                        Box::pin(async move {
                            counter.fetch_add(1, Ordering::SeqCst);
                            Err(RytError::TimeoutError("test error".to_string()))
                        })
                    }
                },
                |error| {
                    // Only retry on HTTP errors
                    matches!(error, RytError::TimeoutError(_))
                },
            )
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 4); // 1 initial + 3 retries
    }

    #[test]
    fn test_retry_executor_default() {
        let _executor = RetryExecutor::default();
        // Test that default executor can be created
        assert!(true); // If we get here, test passed
    }

    #[test]
    fn test_retry_executor_with_config() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 1.5,
            jitter_factor: 0.2,
        };
        let _executor = RetryExecutor::with_config(config);
        // Test that executor with custom config can be created
        assert!(true); // If we get here, test passed
    }

    #[test]
    fn test_retry_config_builder_jitter_clamping() {
        let config = RetryConfigBuilder::new()
            .jitter_factor(1.5) // Should be clamped to 1.0
            .build();

        assert_eq!(config.jitter_factor, 1.0);

        let config = RetryConfigBuilder::new()
            .jitter_factor(-0.5) // Should be clamped to 0.0
            .build();

        assert_eq!(config.jitter_factor, 0.0);
    }

    #[test]
    fn test_retry_config_builder_default() {
        let builder = RetryConfigBuilder::default();
        let config = builder.build();
        
        // Should have default values
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(200));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.jitter_factor, 0.1);
    }

    #[tokio::test]
    async fn test_retry_executor_jitter_zero() {
        let config = RetryConfig {
            max_retries: 1,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter_factor: 0.0, // No jitter
        };
        let executor = RetryExecutor::with_config(config);
        let counter = Arc::new(AtomicU32::new(0));

        let result: Result<String, RytError> = executor
            .execute({
                let counter = counter.clone();
                move || {
                    let counter = counter.clone();
                    Box::pin(async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Err(RytError::TimeoutError("test error".to_string()))
                    })
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 2); // 1 initial + 1 retry
    }

    #[tokio::test]
    async fn test_retry_executor_max_delay_cap() {
        let config = RetryConfig {
            max_retries: 2,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(200), // Low max delay
            backoff_multiplier: 10.0, // High multiplier to exceed max delay
            jitter_factor: 0.0,
        };
        let executor = RetryExecutor::with_config(config);
        let counter = Arc::new(AtomicU32::new(0));

        let result: Result<String, RytError> = executor
            .execute({
                let counter = counter.clone();
                move || {
                    let counter = counter.clone();
                    Box::pin(async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Err(RytError::TimeoutError("test error".to_string()))
                    })
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 3); // 1 initial + 2 retries
    }

    #[tokio::test]
    async fn test_retry_executor_custom_error_handler_non_retryable() {
        let executor = RetryExecutor::new();
        let counter = Arc::new(AtomicU32::new(0));

        let result: Result<String, RytError> = executor
            .execute_with_error_handler(
                {
                    let counter = counter.clone();
                    move || {
                        let counter = counter.clone();
                        Box::pin(async move {
                            counter.fetch_add(1, Ordering::SeqCst);
                            Err(RytError::VideoUnavailable) // Non-retryable error
                        })
                    }
                },
                |error| {
                    // Only retry on timeout errors
                    matches!(error, RytError::TimeoutError(_))
                },
            )
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Only one attempt
    }
}
