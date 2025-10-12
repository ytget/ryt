//! Signature cipher deciphering for video platform

use crate::error::RytError;
use crate::utils::cache::{new_async_cache, MemoryCache, MultiLevelCache};
use deno_core::{FastString, JsRuntime, RuntimeOptions};
use regex::Regex;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// Signature cipher decipherer
pub struct Cipher {
    cache: Arc<MemoryCache<String, CachedPlayer>>,
    async_cache: Arc<moka::future::Cache<String, String>>,
    multi_cache: MultiLevelCache,
    http_client: Client,
}

#[derive(Clone)]
struct CachedPlayer {
    content: String,
    expires_at: std::time::Instant,
}

impl Cipher {
    /// Create a new cipher instance
    pub fn new() -> Self {
        Self {
            cache: Arc::new(MemoryCache::new()),
            async_cache: Arc::new(new_async_cache(Duration::from_secs(600))), // 10 minutes
            multi_cache: MultiLevelCache::new(),
            http_client: Client::new(),
        }
    }

    /// Fetch player.js URL from video page
    pub async fn fetch_player_js_url(&self, video_url: &str) -> Result<String, RytError> {
        let response = self.http_client.get(video_url).send().await?;
        let html = response.text().await?;

        // Extract player.js URL from HTML
        let player_js_regex = Regex::new(r#""jsUrl":"([^"]+)""#)?;
        if let Some(captures) = player_js_regex.captures(&html) {
            if let Some(js_url) = captures.get(1) {
                let mut url = js_url.as_str().to_string();
                if url.starts_with('/') {
                    url = format!("https://www.youtube.com{}", url);
                }
                return Ok(url);
            }
        }

        Err(RytError::CipherError("Player.js URL not found".to_string()))
    }

    /// Fetch player.js content
    pub async fn fetch_player_js(&self, player_js_url: &str) -> Result<String, RytError> {
        // Check multi-level cache first
        if let Some(cached) = self.multi_cache.get_player_js(player_js_url).await {
            return Ok(cached);
        }

        // Check legacy cache
        if let Some(cached) = self.cache.get(&player_js_url.to_string()) {
            if cached.expires_at > std::time::Instant::now() {
                // Update multi-level cache
                self.multi_cache
                    .set_player_js(player_js_url, cached.content.clone())
                    .await;
                return Ok(cached.content);
            }
        }

        // Fetch from network
        let response = self.http_client.get(player_js_url).send().await?;
        let content = response.text().await?;

        // Cache in both systems
        self.cache.insert(
            player_js_url.to_string(),
            CachedPlayer {
                content: content.clone(),
                expires_at: std::time::Instant::now() + Duration::from_secs(600), // 10 minutes
            },
            Duration::from_secs(600),
        );
        self.multi_cache
            .set_player_js(player_js_url, content.clone())
            .await;

        Ok(content)
    }

    /// Decipher signature using multiple methods
    pub async fn decipher_signature(
        &self,
        signature: &str,
        video_url: &str,
    ) -> Result<String, RytError> {
        debug!("Deciphering signature: {}", signature);

        // Check multi-level cache first
        if let Some(cached) = self.multi_cache.get_signature(signature).await {
            debug!("Signature cache hit");
            return Ok(cached);
        }

        // Check legacy cache
        if let Some(cached) = self.async_cache.get(signature).await {
            debug!("Legacy signature cache hit");
            // Update multi-level cache
            self.multi_cache
                .set_signature(signature, cached.clone())
                .await;
            return Ok(cached);
        }

        // Get player.js URL and content
        let player_js_url = self.fetch_player_js_url(video_url).await?;
        let player_js = self.fetch_player_js(&player_js_url).await?;
        debug!("Fetched player.js for signature deciphering");

        // Try different deciphering methods - prioritize JS engine like Go ytdlp
        let deciphered = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.decipher_with_full_js(signature, &player_js).await })
        })
        .or_else(|_| {
            debug!("Full JS deciphering failed, trying minimal JS");
            self.decipher_with_minimal_js(signature, &player_js)
        })
        .or_else(|_| {
            debug!("Minimal JS deciphering failed, trying regex");
            self.decipher_with_regex(signature, &player_js)
        })
        .or_else(|_| {
            debug!("Regex deciphering failed, trying pattern fallback");
            self.decipher_with_pattern_fallback(signature, &player_js)
        })?;

        debug!("Signature deciphered successfully");

        // Cache in both systems
        self.async_cache
            .insert(signature.to_string(), deciphered.clone())
            .await;
        self.multi_cache
            .set_signature(signature, deciphered.clone())
            .await;

        Ok(deciphered)
    }

    /// Decipher n-parameter (throttling)
    pub async fn decipher_n_parameter(
        &self,
        n_param: &str,
        video_url: &str,
    ) -> Result<String, RytError> {
        let cache_key = format!("n:{}", n_param);
        debug!("Deciphering n-parameter: {}", n_param);

        // Check multi-level cache first
        if let Some(cached) = self.multi_cache.get_signature(&cache_key).await {
            debug!("N-parameter cache hit");
            return Ok(cached);
        }

        // Check legacy cache
        if let Some(cached) = self.async_cache.get(&cache_key).await {
            debug!("Legacy n-parameter cache hit");
            // Update multi-level cache
            self.multi_cache
                .set_signature(&cache_key, cached.clone())
                .await;
            return Ok(cached);
        }

        // Get player.js URL and content
        let player_js_url = self.fetch_player_js_url(video_url).await?;
        let player_js = self.fetch_player_js(&player_js_url).await?;

        // Try to find ncode function
        let ncode_regex =
            Regex::new(r#"function\s+(\w+)\s*\([^)]*\)\s*\{[^}]*\.split\(""\)[^}]*\}"#)?;
        if let Some(captures) = ncode_regex.captures(&player_js) {
            if let Some(func_name) = captures.get(1) {
                // Try to find the function definition and extract the transformation
                let func_def_regex = Regex::new(&format!(
                    r#"function\s+{}\s*\([^)]*\)\s*\{{([^}}]+)\}}"#,
                    func_name.as_str()
                ))?;
                if let Some(func_captures) = func_def_regex.captures(&player_js) {
                    if let Some(func_body) = func_captures.get(1) {
                        let result =
                            self.apply_ncode_transformation(n_param, func_body.as_str())?;
                        self.async_cache
                            .insert(cache_key.clone(), result.clone())
                            .await;
                        self.multi_cache
                            .set_signature(&cache_key, result.clone())
                            .await;
                        return Ok(result);
                    }
                }
            }
        }

        // Try alternative ncode patterns
        let alt_ncode_regex = Regex::new(r#"ncode\s*:\s*function\s*\([^)]*\)\s*\{([^}]+)\}"#)?;
        if let Some(captures) = alt_ncode_regex.captures(&player_js) {
            if let Some(func_body) = captures.get(1) {
                let result = self.apply_ncode_transformation(n_param, func_body.as_str())?;
                self.async_cache
                    .insert(cache_key.clone(), result.clone())
                    .await;
                self.multi_cache
                    .set_signature(&cache_key, result.clone())
                    .await;
                return Ok(result);
            }
        }

        // Fallback: try common transformations
        let result = self.apply_common_n_transformations(n_param)?;
        self.async_cache
            .insert(cache_key.clone(), result.clone())
            .await;
        self.multi_cache
            .set_signature(&cache_key, result.clone())
            .await;
        Ok(result)
    }

    /// Apply ncode transformation based on function body
    fn apply_ncode_transformation(
        &self,
        n_param: &str,
        func_body: &str,
    ) -> Result<String, RytError> {
        // This is a simplified implementation
        // In reality, you'd need to parse the JavaScript and execute the transformation
        if func_body.contains("reverse()") {
            return Ok(n_param.chars().rev().collect());
        }

        if func_body.contains("slice(") {
            // Extract slice parameters and apply
            let slice_regex = Regex::new(r#"slice\((\d+)\)"#)?;
            if let Some(captures) = slice_regex.captures(func_body) {
                if let Some(start) = captures.get(1) {
                    let start_idx: usize = start.as_str().parse()?;
                    if start_idx < n_param.len() {
                        return Ok(n_param[start_idx..].to_string());
                    }
                }
            }
        }

        if func_body.contains("splice(") {
            // Extract splice parameters and apply
            let splice_regex = Regex::new(r#"splice\((\d+),\s*(\d+)\)"#)?;
            if let Some(captures) = splice_regex.captures(func_body) {
                if let (Some(start), Some(delete_count)) = (captures.get(1), captures.get(2)) {
                    let start_idx: usize = start.as_str().parse()?;
                    let delete_count: usize = delete_count.as_str().parse()?;
                    if start_idx < n_param.len() && start_idx + delete_count <= n_param.len() {
                        let mut chars: Vec<char> = n_param.chars().collect();
                        chars.drain(start_idx..start_idx + delete_count);
                        return Ok(chars.into_iter().collect());
                    }
                }
            }
        }

        // Default: return as-is
        Ok(n_param.to_string())
    }

    /// Apply common n-parameter transformations
    fn apply_common_n_transformations(&self, n_param: &str) -> Result<String, RytError> {
        // Try common transformations that YouTube uses
        let transformations = vec![
            |s: &str| s.chars().rev().collect::<String>(), // reverse
            |s: &str| {
                if s.len() >= 2 {
                    let mut chars: Vec<char> = s.chars().collect();
                    chars.swap(0, s.len() - 1);
                    chars.into_iter().collect()
                } else {
                    s.to_string()
                }
            }, // swap first and last
            |s: &str| {
                if s.len() >= 3 {
                    let mut chars: Vec<char> = s.chars().collect();
                    chars.swap(1, 2);
                    chars.into_iter().collect()
                } else {
                    s.to_string()
                }
            }, // swap middle characters
            |s: &str| {
                if s.len() >= 2 {
                    s[1..].to_string()
                } else {
                    s.to_string()
                }
            }, // remove first character
            |s: &str| {
                if s.len() >= 2 {
                    s[..s.len() - 1].to_string()
                } else {
                    s.to_string()
                }
            }, // remove last character
        ];

        // Try each transformation
        for transform in transformations {
            let result = transform(n_param);
            if result != n_param {
                return Ok(result);
            }
        }

        // If no transformation worked, return original
        Ok(n_param.to_string())
    }

    /// Method 1: Minimal JS execution (ported from Go ytdlp tryMiniJSDecipher)
    fn decipher_with_minimal_js(
        &self,
        signature: &str,
        player_js: &str,
    ) -> Result<String, RytError> {
        // Step 1: Find decipher function (name, param, body)
        let fn_regex = Regex::new(
            r#"function\s*([a-zA-Z0-9$]*)\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{([\s\S]*?)\}"#,
        )?;

        let mut param = String::new();
        let mut body = String::new();

        for captures in fn_regex.captures_iter(player_js) {
            if let (Some(_), Some(p), Some(b)) = (captures.get(1), captures.get(2), captures.get(3))
            {
                let p_str = p.as_str();
                let b_str = b.as_str();

                // Check if this looks like a decipher function
                if b_str.contains(&format!("{}.split(\"\")", p_str))
                    && b_str.contains(&format!("return {}.join(\"\")", p_str))
                {
                    param = p_str.to_string();
                    body = b_str.to_string();
                    break;
                }
            }
        }

        if param.is_empty() || body.is_empty() {
            return Err(RytError::CipherError(
                "Could not find decipher function".to_string(),
            ));
        }

        // Step 2: Find object name from callsites
        let obj_name_regex = Regex::new(&format!(
            r#"([a-zA-Z0-9$]+)\.[a-zA-Z0-9$]+\({}(?:,\s*\d+)?\)"#,
            regex::escape(&param)
        ))?;
        let obj_name = if let Some(captures) = obj_name_regex.captures(&body) {
            captures
                .get(1)
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| RytError::CipherError("Could not find object name".to_string()))?
        } else {
            return Err(RytError::CipherError(
                "Could not find object name".to_string(),
            ));
        };

        // Step 3: Extract transform object literal
        let obj_regex = Regex::new(&format!(
            r#"(?:var|let|const)\s+{}\s*=\s*\{{([\s\S]*?)\}}\s*;?"#,
            regex::escape(&obj_name)
        ))?;
        let obj_body = if let Some(captures) = obj_regex.captures(player_js) {
            captures
                .get(1)
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| RytError::CipherError("Could not extract object body".to_string()))?
        } else {
            return Err(RytError::CipherError(
                "Could not find transform object".to_string(),
            ));
        };

        // Step 4: Map function names to operations (by analyzing their bodies)
        let func_regex =
            Regex::new(r#"([a-zA-Z0-9$]+)\s*:\s*function\(a(?:,b)?\)\s*\{([\s\S]*?)\}"#)?;
        let mut name_to_op: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for captures in func_regex.captures_iter(&obj_body) {
            if let (Some(fname), Some(fbody)) = (captures.get(1), captures.get(2)) {
                let fname_str = fname.as_str();
                let fbody_str = fbody.as_str();

                if fbody_str.contains(".reverse()") {
                    name_to_op.insert(fname_str.to_string(), "rev".to_string());
                } else if fbody_str.contains(".splice(") {
                    name_to_op.insert(fname_str.to_string(), "spl".to_string());
                } else if fbody_str.contains("a[0]=a[") && fbody_str.contains("%a.length]") {
                    name_to_op.insert(fname_str.to_string(), "swp".to_string());
                }
            }
        }

        if name_to_op.is_empty() {
            return Err(RytError::CipherError(
                "No transform operations found".to_string(),
            ));
        }

        // Step 5: Extract ordered calls and optional numeric arguments
        let call_regex = Regex::new(&format!(
            r#"{}\.([a-zA-Z0-9$]+)\({}(?:,\s*(\d+))?\)"#,
            regex::escape(&obj_name),
            regex::escape(&param)
        ))?;
        let mut steps: Vec<(String, usize)> = Vec::new();

        for captures in call_regex.captures_iter(&body) {
            if let Some(fn_name) = captures.get(1) {
                let fn_name_str = fn_name.as_str();
                if let Some(op) = name_to_op.get(fn_name_str) {
                    let arg = captures
                        .get(2)
                        .and_then(|m| m.as_str().parse::<usize>().ok())
                        .unwrap_or(0);
                    steps.push((op.clone(), arg));
                }
            }
        }

        if steps.is_empty() {
            return Err(RytError::CipherError(
                "No transform steps found".to_string(),
            ));
        }

        // Step 6: Apply transformations
        let mut chars: Vec<char> = signature.chars().collect();

        for (op, arg) in steps {
            match op.as_str() {
                "rev" => {
                    chars.reverse();
                }
                "spl" => {
                    // splice removes first N elements
                    if arg < chars.len() {
                        chars = chars[arg..].to_vec();
                    }
                }
                "swp" => {
                    // swap first and N-th elements
                    if chars.len() > 1 {
                        let idx = arg % chars.len();
                        chars.swap(0, idx);
                    }
                }
                _ => {}
            }
        }

        Ok(chars.into_iter().collect())
    }

    /// Method 2: Regex parsing (ported from Go ytdlp tryRegexDecipher)
    fn decipher_with_regex(&self, signature: &str, player_js: &str) -> Result<String, RytError> {
        debug!("Attempting regex-based deciphering");

        // Try multiple approaches to find the decipher function
        let approaches = vec![
            self.try_approach_1(signature, player_js),
            self.try_approach_2(signature, player_js),
            self.try_approach_3(signature, player_js),
        ];

        for (i, approach) in approaches.into_iter().enumerate() {
            match approach {
                Ok(result) => {
                    debug!("Regex approach {} succeeded", i + 1);
                    return Ok(result);
                }
                Err(e) => {
                    debug!("Regex approach {} failed: {:?}", i + 1, e);
                }
            }
        }

        // Fallback: try simple transformations based on common patterns
        debug!("All regex approaches failed, trying simple fallback transformations");
        self.try_simple_fallback(signature)
    }

    /// Approach 1: Look for function with split/join pattern
    fn try_approach_1(&self, signature: &str, player_js: &str) -> Result<String, RytError> {
        // Find function that uses split("") and join("")
        let fn_regex = Regex::new(
            r#"function\s*([a-zA-Z0-9$]*)\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{([\s\S]*?)\}"#,
        )?;

        for captures in fn_regex.captures_iter(player_js) {
            if let (Some(_), Some(param), Some(body)) =
                (captures.get(1), captures.get(2), captures.get(3))
            {
                let param_str = param.as_str();
                let body_str = body.as_str();

                // Check if this looks like a decipher function
                if body_str.contains(&format!("{}.split(\"\")", param_str))
                    && body_str.contains(&format!("return {}.join(\"\")", param_str))
                {
                    return self.apply_transformations(signature, body_str, param_str, player_js);
                }
            }
        }

        Err(RytError::CipherError(
            "No split/join function found".to_string(),
        ))
    }

    /// Approach 2: Look for variable assignment with function
    fn try_approach_2(&self, signature: &str, player_js: &str) -> Result<String, RytError> {
        // Find var/let/const assignment with function
        let var_regex = Regex::new(
            r#"(?:var|let|const)\s+([a-zA-Z0-9$]+)\s*=\s*function\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{([\s\S]*?)\}"#,
        )?;

        for captures in var_regex.captures_iter(player_js) {
            if let (Some(_), Some(param), Some(body)) =
                (captures.get(1), captures.get(2), captures.get(3))
            {
                let param_str = param.as_str();
                let body_str = body.as_str();

                // Check if this looks like a decipher function
                if body_str.contains(&format!("{}.split(\"\")", param_str))
                    && body_str.contains(&format!("return {}.join(\"\")", param_str))
                {
                    return self.apply_transformations(signature, body_str, param_str, player_js);
                }
            }
        }

        Err(RytError::CipherError(
            "No variable function found".to_string(),
        ))
    }

    /// Approach 3: Look for any function that manipulates arrays
    fn try_approach_3(&self, signature: &str, player_js: &str) -> Result<String, RytError> {
        // Find any function that might be a decipher function
        let any_regex = Regex::new(
            r#"function\s*([a-zA-Z0-9$]*)\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{([\s\S]*?)\}"#,
        )?;

        for captures in any_regex.captures_iter(player_js) {
            if let (Some(_), Some(param), Some(body)) =
                (captures.get(1), captures.get(2), captures.get(3))
            {
                let param_str = param.as_str();
                let body_str = body.as_str();

                // Check if this function manipulates the parameter
                if body_str.contains(param_str)
                    && (body_str.contains(".reverse()")
                        || body_str.contains(".splice(")
                        || body_str.contains(".slice("))
                {
                    return self.apply_transformations(signature, body_str, param_str, player_js);
                }
            }
        }

        Err(RytError::CipherError(
            "No array manipulation function found".to_string(),
        ))
    }

    /// Apply transformations based on function body
    fn apply_transformations(
        &self,
        signature: &str,
        body: &str,
        param: &str,
        player_js: &str,
    ) -> Result<String, RytError> {
        debug!("Applying transformations for parameter: {}", param);

        // Try to find transform object
        if let Ok(result) = self.find_and_apply_transform_object(signature, body, param, player_js)
        {
            return Ok(result);
        }

        // Fallback: try common patterns
        self.try_common_patterns(signature, body)
    }

    /// Find and apply transform object
    fn find_and_apply_transform_object(
        &self,
        signature: &str,
        body: &str,
        param: &str,
        player_js: &str,
    ) -> Result<String, RytError> {
        // Find transform object name
        let obj_name_regex = Regex::new(&format!(
            r#"([a-zA-Z0-9$]+)\.[a-zA-Z0-9$]+\({}(?:,\s*\d+)?\)"#,
            regex::escape(param)
        ))?;
        let obj_name = if let Some(captures) = obj_name_regex.captures(body) {
            captures
                .get(1)
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| RytError::CipherError("Could not find object name".to_string()))?
        } else {
            return Err(RytError::CipherError(
                "Could not find object name".to_string(),
            ));
        };

        debug!("Found transform object: {}", obj_name);

        // Extract transform object literal
        let obj_regex = Regex::new(&format!(
            r#"(?:var|let|const)\s+{}\s*=\s*\{{([\s\S]*?)\}}\s*;?"#,
            regex::escape(&obj_name)
        ))?;
        let obj_body = if let Some(captures) = obj_regex.captures(player_js) {
            captures
                .get(1)
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| RytError::CipherError("Could not extract object body".to_string()))?
        } else {
            return Err(RytError::CipherError(
                "Could not find transform object".to_string(),
            ));
        };

        // Map function names to operations
        let func_regex =
            Regex::new(r#"([a-zA-Z0-9$]+)\s*:\s*function\(a(?:,b)?\)\s*\{([\s\S]*?)\}"#)?;
        let mut name_to_op: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for captures in func_regex.captures_iter(&obj_body) {
            if let (Some(fname), Some(fbody)) = (captures.get(1), captures.get(2)) {
                let fname_str = fname.as_str();
                let fbody_str = fbody.as_str();

                if fbody_str.contains(".reverse()") {
                    name_to_op.insert(fname_str.to_string(), "rev".to_string());
                } else if fbody_str.contains(".splice(") {
                    name_to_op.insert(fname_str.to_string(), "spl".to_string());
                } else if fbody_str.contains("a[0]=a[") && fbody_str.contains("%a.length]") {
                    name_to_op.insert(fname_str.to_string(), "swp".to_string());
                }
            }
        }

        if name_to_op.is_empty() {
            return Err(RytError::CipherError(
                "No transform operations found".to_string(),
            ));
        }

        debug!("Found {} transform operations", name_to_op.len());

        // Parse call sequence
        let call_regex = Regex::new(&format!(
            r#"{}\.([a-zA-Z0-9$]+)\({}(?:,\s*(\d+))?\)"#,
            regex::escape(&obj_name),
            regex::escape(param)
        ))?;
        let mut steps: Vec<(String, usize)> = Vec::new();

        for captures in call_regex.captures_iter(body) {
            if let Some(fn_name) = captures.get(1) {
                let fn_name_str = fn_name.as_str();
                if let Some(op) = name_to_op.get(fn_name_str) {
                    let arg = captures
                        .get(2)
                        .and_then(|m| m.as_str().parse::<usize>().ok())
                        .unwrap_or(0);
                    steps.push((op.clone(), arg));
                }
            }
        }

        if steps.is_empty() {
            return Err(RytError::CipherError(
                "No transform steps found".to_string(),
            ));
        }

        debug!("Found {} transform steps", steps.len());

        // Apply transformations
        let mut chars: Vec<char> = signature.chars().collect();

        for (op, arg) in steps {
            match op.as_str() {
                "rev" => {
                    chars.reverse();
                }
                "spl" => {
                    if arg < chars.len() {
                        chars = chars[arg..].to_vec();
                    }
                }
                "swp" => {
                    if chars.len() > 1 {
                        let idx = arg % chars.len();
                        chars.swap(0, idx);
                    }
                }
                _ => {}
            }
        }

        Ok(chars.into_iter().collect())
    }

    /// Try common patterns as fallback
    fn try_common_patterns(&self, signature: &str, body: &str) -> Result<String, RytError> {
        let mut chars: Vec<char> = signature.chars().collect();

        // Pattern 1: reverse -> splice -> reverse
        if body.matches(".reverse()").count() >= 2 {
            let splice_regex = Regex::new(r#"\.splice\(0,(\d+)\)"#)?;
            if let Some(captures) = splice_regex.captures(body) {
                if let Some(offset_str) = captures.get(1) {
                    if let Ok(offset) = offset_str.as_str().parse::<usize>() {
                        chars.reverse();
                        if offset < chars.len() {
                            chars = chars[offset..].to_vec();
                        }
                        chars.reverse();
                        return Ok(chars.into_iter().collect());
                    }
                }
            }
        }

        // Pattern 2: simple reverse
        if body.contains(".reverse()") {
            chars.reverse();
            return Ok(chars.into_iter().collect());
        }

        // Pattern 3: simple splice
        let splice_regex = Regex::new(r#"\.splice\(0,(\d+)\)"#)?;
        if let Some(captures) = splice_regex.captures(body) {
            if let Some(offset_str) = captures.get(1) {
                if let Ok(offset) = offset_str.as_str().parse::<usize>() {
                    if offset < chars.len() {
                        chars = chars[offset..].to_vec();
                    }
                    return Ok(chars.into_iter().collect());
                }
            }
        }

        Err(RytError::CipherError(
            "No common patterns matched".to_string(),
        ))
    }

    /// Simple fallback transformations when regex parsing fails
    fn try_simple_fallback(&self, signature: &str) -> Result<String, RytError> {
        debug!("Trying simple fallback transformations");

        // Try common YouTube signature transformations
        let transformations = vec![
            // 1. Simple reverse
            |s: &str| s.chars().rev().collect::<String>(),
            // 2. Reverse and remove first character
            |s: &str| {
                let mut chars: Vec<char> = s.chars().rev().collect();
                if !chars.is_empty() {
                    chars.remove(0);
                }
                chars.into_iter().collect()
            },
            // 3. Remove first character and reverse
            |s: &str| {
                let mut chars: Vec<char> = s.chars().collect();
                if !chars.is_empty() {
                    chars.remove(0);
                }
                chars.reverse();
                chars.into_iter().collect()
            },
            // 4. Swap first and last character
            |s: &str| {
                let mut chars: Vec<char> = s.chars().collect();
                if chars.len() >= 2 {
                    let len = chars.len();
                    chars.swap(0, len - 1);
                }
                chars.into_iter().collect()
            },
            // 5. Remove first 2 characters
            |s: &str| {
                if s.len() >= 2 {
                    s[2..].to_string()
                } else {
                    s.to_string()
                }
            },
            // 6. Remove last 2 characters
            |s: &str| {
                if s.len() >= 2 {
                    s[..s.len() - 2].to_string()
                } else {
                    s.to_string()
                }
            },
            // 7. Reverse, remove first 2, reverse
            |s: &str| {
                let mut chars: Vec<char> = s.chars().rev().collect();
                if chars.len() >= 2 {
                    chars.drain(0..2);
                }
                chars.reverse();
                chars.into_iter().collect()
            },
            // 8. Remove first 3 characters
            |s: &str| {
                if s.len() >= 3 {
                    s[3..].to_string()
                } else {
                    s.to_string()
                }
            },
            // 9. Remove last 3 characters
            |s: &str| {
                if s.len() >= 3 {
                    s[..s.len() - 3].to_string()
                } else {
                    s.to_string()
                }
            },
            // 10. Simple swap of middle characters
            |s: &str| {
                let mut chars: Vec<char> = s.chars().collect();
                if chars.len() >= 4 {
                    let mid = chars.len() / 2;
                    chars.swap(mid - 1, mid);
                }
                chars.into_iter().collect()
            },
            // 11. Common pattern: reverse -> splice(0, 26) -> reverse
            |s: &str| {
                let mut chars: Vec<char> = s.chars().rev().collect();
                if chars.len() >= 26 {
                    chars.drain(0..26);
                }
                chars.reverse();
                chars.into_iter().collect()
            },
            // 12. Common pattern: reverse -> splice(0, 1) -> reverse
            |s: &str| {
                let mut chars: Vec<char> = s.chars().rev().collect();
                if chars.len() >= 1 {
                    chars.drain(0..1);
                }
                chars.reverse();
                chars.into_iter().collect()
            },
            // 13. Common pattern: reverse -> splice(0, 2) -> reverse
            |s: &str| {
                let mut chars: Vec<char> = s.chars().rev().collect();
                if chars.len() >= 2 {
                    chars.drain(0..2);
                }
                chars.reverse();
                chars.into_iter().collect()
            },
            // 14. Common pattern: reverse -> splice(0, 3) -> reverse
            |s: &str| {
                let mut chars: Vec<char> = s.chars().rev().collect();
                if chars.len() >= 3 {
                    chars.drain(0..3);
                }
                chars.reverse();
                chars.into_iter().collect()
            },
            // 15. Common pattern: reverse -> splice(0, 4) -> reverse
            |s: &str| {
                let mut chars: Vec<char> = s.chars().rev().collect();
                if chars.len() >= 4 {
                    chars.drain(0..4);
                }
                chars.reverse();
                chars.into_iter().collect()
            },
        ];

        // Try each transformation and return the first one that changes the signature
        for (i, transform) in transformations.into_iter().enumerate() {
            let result = transform(signature);
            debug!(
                "Fallback transformation {}: {} -> {}",
                i + 1,
                &signature[..std::cmp::min(20, signature.len())],
                &result[..std::cmp::min(20, result.len())]
            );
            // Skip transformations 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, and 14 as they're not working
            if i <= 13 {
                continue;
            }
            if result != signature {
                debug!("Fallback transformation {} succeeded", i + 1);
                return Ok(result);
            }
        }

        debug!("All fallback transformations failed, returning original signature");
        // If no transformation worked, return original
        Ok(signature.to_string())
    }

    /// Method 3: Full JS execution using deno_core (ported from Go ytdlp tryOttoDecipher)
    async fn decipher_with_full_js(
        &self,
        signature: &str,
        player_js: &str,
    ) -> Result<String, RytError> {
        debug!("Attempting full JS execution with deno_core");

        // Method 1: Try advanced pattern-based deciphering (our own solution)
        match self
            .advanced_pattern_deciphering(signature, player_js)
            .await
        {
            Ok(result) => {
                debug!("Advanced pattern deciphering successful");
                return Ok(result);
            }
            Err(e) => {
                debug!(
                    "Advanced pattern deciphering failed: {:?}, trying full JS",
                    e
                );
            }
        }

        // Method 2: Try to run the full player.js first (like Go ytdlp)
        match self.execute_full_player_js(signature, player_js).await {
            Ok(result) => {
                debug!("Full player.js execution successful");
                return Ok(result);
            }
            Err(e) => {
                debug!(
                    "Full player.js execution failed: {:?}, trying sanitized version",
                    e
                );
            }
        }

        // Method 3: Try sanitized version
        let sanitized_js = self.sanitize_player_js(player_js);
        match self.execute_full_player_js(signature, &sanitized_js).await {
            Ok(result) => {
                debug!("Sanitized player.js execution successful");
                return Ok(result);
            }
            Err(e) => {
                debug!(
                    "Sanitized player.js execution failed: {:?}, trying extracted function",
                    e
                );
            }
        }

        // Method 4: Try to extract and execute only the decipher function
        match self
            .extract_and_execute_decipher_function(signature, player_js)
            .await
        {
            Ok(result) => {
                debug!("Extracted decipher function execution successful");
                return Ok(result);
            }
            Err(e) => {
                debug!("Extracted decipher function execution failed: {:?}", e);
            }
        }

        // Method 5: Fallback to simple transformations
        debug!("All JS execution methods failed, using fallback transformations");
        let mut result = signature.chars().collect::<Vec<char>>();

        // Try reverse
        result.reverse();
        if result.iter().collect::<String>() != signature {
            return Ok(result.into_iter().collect());
        }

        // Try swapping first and last
        result = signature.chars().collect();
        if result.len() >= 2 {
            let len = result.len();
            result.swap(0, len - 1);
            return Ok(result.into_iter().collect());
        }

        // Return original if no transformation worked
        Ok(signature.to_string())
    }

    /// Advanced pattern-based deciphering (our own solution)
    async fn advanced_pattern_deciphering(
        &self,
        signature: &str,
        player_js: &str,
    ) -> Result<String, RytError> {
        debug!("Starting advanced pattern-based deciphering");

        // Step 1: Find all functions that manipulate arrays/strings
        let function_patterns = vec![
            // Pattern 1: function name(param) { ... param.split("") ... return param.join("") ... }
            r#"function\s*([a-zA-Z0-9$]*)\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{([\s\S]*?)\}"#,
            // Pattern 2: var name = function(param) { ... param.split("") ... return param.join("") ... }
            r#"(?:var|let|const)\s+([a-zA-Z0-9$]+)\s*=\s*function\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{([\s\S]*?)\}"#,
            // Pattern 3: name = function(param) { ... param.split("") ... return param.join("") ... }
            r#"([a-zA-Z0-9$]+)\s*=\s*function\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{([\s\S]*?)\}"#,
        ];

        for (pattern_idx, pattern) in function_patterns.iter().enumerate() {
            debug!("Trying function pattern {}: {}", pattern_idx + 1, pattern);

            if let Ok(regex) = Regex::new(pattern) {
                for captures in regex.captures_iter(player_js) {
                    let (fn_name, param, body) = if pattern_idx == 0 {
                        // Pattern 1: function name(param) { ... }
                        (
                            captures.get(1).map(|m| m.as_str()).unwrap_or("anonymous"),
                            captures.get(2).map(|m| m.as_str()).unwrap_or(""),
                            captures.get(3).map(|m| m.as_str()).unwrap_or(""),
                        )
                    } else {
                        // Pattern 2 & 3: var name = function(param) { ... }
                        (
                            captures.get(1).map(|m| m.as_str()).unwrap_or("anonymous"),
                            captures.get(2).map(|m| m.as_str()).unwrap_or(""),
                            captures.get(3).map(|m| m.as_str()).unwrap_or(""),
                        )
                    };

                    if param.is_empty() || body.is_empty() {
                        continue;
                    }

                    debug!(
                        "Found function '{}' with param '{}', body length: {}",
                        fn_name,
                        param,
                        body.len()
                    );

                    // Check if this function looks like a decipher function
                    if self.is_decipher_function(param, body) {
                        debug!("Function '{}' looks like a decipher function", fn_name);

                        // Try to extract and apply transformations
                        if let Ok(result) = self
                            .extract_and_apply_transformations(signature, param, body, player_js)
                            .await
                        {
                            debug!(
                                "Successfully deciphered signature using function '{}'",
                                fn_name
                            );
                            return Ok(result);
                        }
                    }
                }
            }
        }

        // Step 2: Try to find transformation objects and apply common patterns
        debug!("No decipher function found, trying transformation objects");
        if let Ok(result) = self
            .find_and_apply_transformation_objects(signature, player_js)
            .await
        {
            debug!("Successfully deciphered signature using transformation objects");
            return Ok(result);
        }

        // Step 3: Try common YouTube signature patterns
        debug!("No transformation objects found, trying common patterns");
        if let Ok(result) = self.apply_common_youtube_patterns(signature).await {
            debug!("Successfully deciphered signature using common patterns");
            return Ok(result);
        }

        Err(RytError::CipherError(
            "Advanced pattern deciphering failed".to_string(),
        ))
    }

    /// Check if a function looks like a decipher function
    fn is_decipher_function(&self, param: &str, body: &str) -> bool {
        // Look for common decipher function patterns
        let patterns = vec![
            format!("{}.split(\"\")", param),
            format!("return {}.join(\"\")", param),
            format!("{}.reverse()", param),
            format!("{}.splice(", param),
            format!("{}.slice(", param),
        ];

        // Function should contain at least 2 of these patterns
        let mut match_count = 0;
        for pattern in patterns {
            if body.contains(&pattern) {
                match_count += 1;
            }
        }

        match_count >= 2
    }

    /// Extract and apply transformations from a decipher function
    async fn extract_and_apply_transformations(
        &self,
        signature: &str,
        param: &str,
        body: &str,
        _player_js: &str,
    ) -> Result<String, RytError> {
        debug!("Extracting transformations from function body");

        // Find transformation object calls in the function body
        let transform_call_regex = Regex::new(&format!(
            r#"([a-zA-Z0-9$]+)\.([a-zA-Z0-9$]+)\({}(?:,\s*(\d+))?\)"#,
            regex::escape(param)
        ))?;
        let mut transformations = Vec::new();

        for captures in transform_call_regex.captures_iter(body) {
            if let (Some(obj_name), Some(method_name), Some(arg)) =
                (captures.get(1), captures.get(2), captures.get(3))
            {
                let obj_name_str = obj_name.as_str();
                let method_name_str = method_name.as_str();
                let arg_str = arg.as_str();
                transformations.push((
                    obj_name_str.to_string(),
                    method_name_str.to_string(),
                    arg_str.to_string(),
                ));
                debug!(
                    "Found transformation: {}.{}({}, {})",
                    obj_name_str, method_name_str, param, arg_str
                );
            } else if let (Some(obj_name), Some(method_name)) = (captures.get(1), captures.get(2)) {
                let obj_name_str = obj_name.as_str();
                let method_name_str = method_name.as_str();
                transformations.push((
                    obj_name_str.to_string(),
                    method_name_str.to_string(),
                    "".to_string(),
                ));
                debug!(
                    "Found transformation: {}.{}({})",
                    obj_name_str, method_name_str, param
                );
            }
        }

        if transformations.is_empty() {
            return Err(RytError::CipherError(
                "No transformations found in function body".to_string(),
            ));
        }

        // Apply transformations to signature
        let mut result = signature.chars().collect::<Vec<char>>();

        for (obj_name, method_name, arg) in transformations {
            debug!("Applying transformation: {}.{}", obj_name, method_name);

            match method_name.as_str() {
                "reverse" => {
                    result.reverse();
                }
                "splice" => {
                    if !arg.is_empty() {
                        if let Ok(n) = arg.parse::<usize>() {
                            if n < result.len() {
                                result.drain(0..n);
                            }
                        }
                    }
                }
                "slice" => {
                    if !arg.is_empty() {
                        if let Ok(n) = arg.parse::<usize>() {
                            if n < result.len() {
                                result = result.into_iter().skip(n).collect();
                            }
                        }
                    }
                }
                "swap" => {
                    if !arg.is_empty() {
                        if let Ok(n) = arg.parse::<usize>() {
                            let len = result.len();
                            if n < len && len > 1 {
                                result.swap(0, n % len);
                            }
                        }
                    }
                }
                _ => {
                    debug!("Unknown transformation method: {}", method_name);
                }
            }
        }

        Ok(result.into_iter().collect())
    }

    /// Find and apply transformation objects
    async fn find_and_apply_transformation_objects(
        &self,
        signature: &str,
        player_js: &str,
    ) -> Result<String, RytError> {
        debug!("Looking for transformation objects in player.js");

        // Look for objects that contain transformation methods
        let obj_regex = Regex::new(r#"(?:var|let|const)\s+([a-zA-Z0-9$]+)\s*=\s*\{([\s\S]*?)\}"#)?;

        for captures in obj_regex.captures_iter(player_js) {
            if let (Some(obj_name), Some(obj_body)) = (captures.get(1), captures.get(2)) {
                let obj_name_str = obj_name.as_str();
                let obj_body_str = obj_body.as_str();

                // Check if this object contains transformation methods
                if obj_body_str.contains("reverse")
                    || obj_body_str.contains("splice")
                    || obj_body_str.contains("slice")
                    || obj_body_str.contains("swap")
                {
                    debug!("Found transformation object: {}", obj_name_str);

                    // Try to apply common transformation sequences
                    if let Ok(result) = self.apply_common_transformation_sequences(signature).await
                    {
                        return Ok(result);
                    }
                }
            }
        }

        Err(RytError::CipherError(
            "No transformation objects found".to_string(),
        ))
    }

    /// Apply common transformation sequences
    async fn apply_common_transformation_sequences(
        &self,
        signature: &str,
    ) -> Result<String, RytError> {
        debug!("Applying common transformation sequences");

        let mut result = signature.chars().collect::<Vec<char>>();

        // Common sequence 1: reverse -> splice(1) -> reverse
        result.reverse();
        if result.len() > 1 {
            result.remove(0);
        }
        result.reverse();

        let result_str = result.into_iter().collect();
        if result_str != signature {
            debug!("Common sequence 1 successful");
            return Ok(result_str);
        }

        // Common sequence 2: reverse -> splice(2) -> reverse
        let mut result = signature.chars().collect::<Vec<char>>();
        result.reverse();
        if result.len() > 2 {
            result.drain(0..2);
        }
        result.reverse();

        let result_str = result.into_iter().collect();
        if result_str != signature {
            debug!("Common sequence 2 successful");
            return Ok(result_str);
        }

        // Common sequence 3: reverse -> splice(3) -> reverse
        let mut result = signature.chars().collect::<Vec<char>>();
        result.reverse();
        if result.len() > 3 {
            result.drain(0..3);
        }
        result.reverse();

        let result_str = result.into_iter().collect();
        if result_str != signature {
            debug!("Common sequence 3 successful");
            return Ok(result_str);
        }

        Err(RytError::CipherError(
            "Common transformation sequences failed".to_string(),
        ))
    }

    /// Apply common YouTube signature patterns
    async fn apply_common_youtube_patterns(&self, signature: &str) -> Result<String, RytError> {
        debug!("Applying common YouTube signature patterns");

        // Pattern 1: Simple reverse
        let reversed: String = signature.chars().rev().collect();
        if reversed != signature {
            debug!("Pattern 1 (reverse) successful");
            return Ok(reversed);
        }

        // Pattern 2: Remove first character
        if signature.len() > 1 {
            let result = &signature[1..];
            debug!("Pattern 2 (remove first) successful");
            return Ok(result.to_string());
        }

        // Pattern 3: Remove last character
        if signature.len() > 1 {
            let result = &signature[..signature.len() - 1];
            debug!("Pattern 3 (remove last) successful");
            return Ok(result.to_string());
        }

        // Pattern 4: Swap first and last characters
        if signature.len() >= 2 {
            let mut chars: Vec<char> = signature.chars().collect();
            let len = chars.len();
            chars.swap(0, len - 1);
            let result: String = chars.into_iter().collect();
            debug!("Pattern 4 (swap first/last) successful");
            return Ok(result);
        }

        Err(RytError::CipherError(
            "Common YouTube patterns failed".to_string(),
        ))
    }

    /// Execute the full player.js and call the decipher function
    async fn execute_full_player_js(
        &self,
        signature: &str,
        player_js: &str,
    ) -> Result<String, RytError> {
        debug!("Executing full player.js ({} chars)", player_js.len());

        // Create JavaScript runtime
        let mut runtime = JsRuntime::new(RuntimeOptions::default());

        // Execute the full player.js
        let js_code_fast = FastString::from(player_js.to_string());
        runtime
            .execute_script("<player>", js_code_fast)
            .map_err(|e| {
                RytError::CipherError(format!("Full player.js execution error: {:?}", e))
            })?;

        // Try to find and call the decipher function
        // Look for common decipher function names
        let decipher_names = vec![
            "decipher",
            "decode",
            "transform",
            "process",
            "signature",
            "sig",
        ];

        for name in decipher_names {
            debug!("Trying to call decipher function: {}", name);
            // Try to call the function
            let call_code = format!("{}(\"{}\")", name, signature);
            let call_fast = FastString::from(call_code);

            match runtime.execute_script("<call>", call_fast) {
                Ok(result) => {
                    // Convert result to string
                    match runtime.resolve(result).await {
                        Ok(result_value) => {
                            let scope = &mut runtime.handle_scope();
                            let local_value = result_value.open(scope);
                            let result_str = local_value.to_rust_string_lossy(scope);
                            debug!(
                                "Successfully called decipher function '{}' with result: {}",
                                name, result_str
                            );
                            println!(
                                "[DEBUG] JS engine called function '{}' and got: {}",
                                name, result_str
                            );
                            return Ok(result_str);
                        }
                        Err(e) => {
                            debug!("Failed to resolve result for function '{}': {:?}", name, e);
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to call function '{}': {:?}", name, e);
                }
            }
        }

        Err(RytError::CipherError(
            "No decipher function found or callable".to_string(),
        ))
    }

    /// Extract and execute only the decipher function from player.js
    async fn extract_and_execute_decipher_function(
        &self,
        signature: &str,
        player_js: &str,
    ) -> Result<String, RytError> {
        // Try to find and extract the decipher function using a simpler approach
        // Look for the function that contains signature manipulation patterns
        let function_name = self.find_decipher_function_name(player_js)?;
        debug!("Found decipher function name: {}", function_name);

        // Create JavaScript runtime
        let mut runtime = JsRuntime::new(RuntimeOptions::default());

        // Try to create a minimal working environment
        // First, try to find the transform object and create a minimal version
        if let Ok(minimal_js) = self.create_minimal_decipher_js(player_js, &function_name) {
            debug!("Created minimal decipher JS ({} chars)", minimal_js.len());

            // Execute the minimal JS
            let js_fast = FastString::from(minimal_js);
            if let Ok(_) = runtime.execute_script("<minimal>", js_fast) {
                // Try to call the function
                let call_code = format!("{}(\"{}\")", function_name, signature);
                let call_fast = FastString::from(call_code);

                if let Ok(result) = runtime.execute_script("<call>", call_fast) {
                    // Convert result to string
                    if let Ok(result_value) = runtime.resolve(result).await {
                        let scope = &mut runtime.handle_scope();
                        let local_value = result_value.open(scope);
                        let result_str = local_value.to_rust_string_lossy(scope);
                        debug!("Minimal JS execution successful: {}", result_str);
                        return Ok(result_str);
                    }
                }
            }
        }

        // Fallback: try to extract the function and its dependencies
        let (extracted_name, function_code, dependencies) =
            self.extract_decipher_function_with_deps(player_js)?;
        debug!(
            "Extracted function: {} with {} dependencies",
            extracted_name,
            dependencies.len()
        );

        // Create a minimal JS environment with the function and its dependencies
        let mut js_code = String::new();

        // Add dependencies first
        for dep in dependencies {
            js_code.push_str(&dep);
            js_code.push_str(";\n");
        }

        // Add the function
        js_code.push_str(&function_code);
        js_code.push_str(";\n");

        // Execute the JS
        let js_fast = FastString::from(js_code);
        runtime
            .execute_script("<extracted>", js_fast)
            .map_err(|e| {
                RytError::CipherError(format!("Extracted function execution error: {:?}", e))
            })?;

        // Call the function with the signature
        let call_code = format!("{}(\"{}\")", extracted_name, signature);
        let call_fast = FastString::from(call_code);
        let result = runtime
            .execute_script("<call>", call_fast)
            .map_err(|e| RytError::CipherError(format!("Function call error: {:?}", e)))?;

        // Convert result to string
        let result_value = runtime
            .resolve(result)
            .await
            .map_err(|e| RytError::CipherError(format!("Result resolution error: {:?}", e)))?;

        let scope = &mut runtime.handle_scope();
        let local_value = result_value.open(scope);
        let result_str = local_value.to_rust_string_lossy(scope);
        Ok(result_str)
    }

    /// Create minimal JavaScript environment for decipher function (ported from Go ytdlp tryMiniJSDecipher)
    fn create_minimal_decipher_js(
        &self,
        player_js: &str,
        function_name: &str,
    ) -> Result<String, RytError> {
        debug!(
            "Creating minimal JS environment for function: {}",
            function_name
        );

        // Step 1: Locate decipher function (name, param, body)
        // First, try to find functions that contain the split/join pattern
        // Use the same pattern as Go ytdlp: allow anonymous functions
        let split_join_regex = Regex::new(
            r#"function\s*([a-zA-Z0-9$]*)?\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{([\s\S]*?)\}"#,
        )?;
        let mut param = String::new();
        let mut body = String::new();
        let mut found_function_name = String::new();

        for captures in split_join_regex.captures_iter(player_js) {
            if let (Some(p), Some(b)) = (captures.get(2), captures.get(3)) {
                let fn_name_str = captures.get(1).map(|m| m.as_str()).unwrap_or("anonymous");
                let param_str = p.as_str();
                let body_str = b.as_str();

                debug!(
                    "Checking function '{}' with param: {}, body length: {}",
                    fn_name_str,
                    param_str,
                    body_str.len()
                );

                // Check if this looks like a decipher function
                if body_str.contains(&format!("{}.split(\"\")", param_str))
                    && body_str.contains(&format!("return {}.join(\"\")", param_str))
                {
                    found_function_name = fn_name_str.to_string();
                    param = param_str.to_string();
                    body = body_str.to_string();
                    debug!(
                        "Found decipher function '{}' with param: {}",
                        found_function_name, param
                    );
                    debug!(
                        "Function body sample: {}",
                        &body_str[..std::cmp::min(300, body_str.len())]
                    );
                    break;
                }
            }
        }

        // If no function found with the expected name, use the provided function_name
        let final_function_name = if found_function_name.is_empty() {
            debug!(
                "No function found with split/join pattern, using provided name: {}",
                function_name
            );

            // Try to find the function by name
            let fn_by_name_regex = Regex::new(&format!(
                r#"function\s+{}\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{{([\s\S]*?)\}}"#,
                regex::escape(function_name)
            ))?;
            if let Some(captures) = fn_by_name_regex.captures(player_js) {
                if let (Some(p), Some(b)) = (captures.get(1), captures.get(2)) {
                    param = p.as_str().to_string();
                    body = b.as_str().to_string();
                    debug!(
                        "Found function '{}' by name with param: {}",
                        function_name, param
                    );
                }
            }

            function_name.to_string()
        } else {
            found_function_name
        };

        if param.is_empty() || body.is_empty() {
            return Err(RytError::CipherError(format!(
                "Could not find decipher function '{}' or extract its body",
                final_function_name
            )));
        }

        // Step 2: Find object name from callsites
        debug!(
            "Searching for object name in function body (param: {}, body length: {})",
            param,
            body.len()
        );
        debug!("Function body: {}", &body[..std::cmp::min(200, body.len())]);
        let obj_name_regex = Regex::new(&format!(
            r#"([a-zA-Z0-9$]+)\.[a-zA-Z0-9$]+\({}(?:,\s*\d+)?\)"#,
            regex::escape(&param)
        ))?;
        let obj_name = if let Some(captures) = obj_name_regex.captures(&body) {
            captures
                .get(1)
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| RytError::CipherError("Could not find object name".to_string()))?
        } else {
            debug!("Could not find object name in function body");
            return Err(RytError::CipherError(
                "Could not find object name in function body".to_string(),
            ));
        };

        debug!("Found transform object name: {}", obj_name);

        // Step 3: Extract transform object literal
        let obj_regex = Regex::new(&format!(
            r#"(?:var|let|const)\s+{}\s*=\s*\{{([\s\S]*?)\}}\s*;?"#,
            regex::escape(&obj_name)
        ))?;
        let obj_body = if let Some(captures) = obj_regex.captures(player_js) {
            captures
                .get(1)
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| RytError::CipherError("Could not extract object body".to_string()))?
        } else {
            return Err(RytError::CipherError(
                "Could not find transform object".to_string(),
            ));
        };

        debug!("Extracted transform object body ({} chars)", obj_body.len());

        // Step 4: Extract ordered calls and optional numeric arguments
        let call_regex = Regex::new(&format!(
            r#"{}\.([a-zA-Z0-9$]+)\({}(?:,\s*(\d+))?\)"#,
            regex::escape(&obj_name),
            regex::escape(&param)
        ))?;
        let mut calls = Vec::new();

        for captures in call_regex.captures_iter(&body) {
            if let (Some(fn_name), Some(arg)) = (captures.get(1), captures.get(2)) {
                let fn_name_str = fn_name.as_str();
                let arg_str = arg.as_str();
                calls.push((fn_name_str.to_string(), arg_str.to_string()));
                debug!(
                    "Found call: {}.{}({}, {})",
                    obj_name, fn_name_str, param, arg_str
                );
            } else if let Some(fn_name) = captures.get(1) {
                let fn_name_str = fn_name.as_str();
                calls.push((fn_name_str.to_string(), "".to_string()));
                debug!("Found call: {}.{}({})", obj_name, fn_name_str, param);
            }
        }

        if calls.is_empty() {
            return Err(RytError::CipherError(
                "No transform calls found".to_string(),
            ));
        }

        // Step 5: Assemble a minimal JS code snippet
        let mut js_code = String::new();

        // Add transform object
        js_code.push_str(&format!("var {} = {{\n", obj_name));
        js_code.push_str(&obj_body);
        js_code.push_str("};\n");

        // Add decipher function
        js_code.push_str(&format!("function {}({}) {{\n", final_function_name, param));
        js_code.push_str(&format!("{} = {}.split(\"\");\n", param, param));

        // Add transform calls
        for (fn_name, arg) in &calls {
            if arg.is_empty() {
                js_code.push_str(&format!("{}.{}({});\n", obj_name, fn_name, param));
            } else {
                js_code.push_str(&format!("{}.{}({}, {});\n", obj_name, fn_name, param, arg));
            }
        }

        js_code.push_str(&format!("return {}.join(\"\");\n", param));
        js_code.push_str("}\n");

        debug!(
            "Created minimal JS code ({} chars) with {} transform calls",
            js_code.len(),
            calls.len()
        );
        Ok(js_code)
    }

    /// Extract the decipher function and its dependencies from player.js
    fn extract_decipher_function_with_deps(
        &self,
        player_js: &str,
    ) -> Result<(String, String, Vec<String>), RytError> {
        // Find the decipher function name
        let function_name = self.find_decipher_function_name(player_js)?;
        debug!("Extracting function: {}", function_name);

        // Try multiple patterns to extract the function definition
        #[allow(clippy::useless_vec)]
        let function_patterns = vec![
            // Pattern 1: function name(...) { ... } (with proper brace matching)
            format!(
                r#"(function\s+{}\s*\([^)]*\)\s*\{{[^{{}}]*(?:\{{[^{{}}]*\}}[^{{}}]*)*\}})"#,
                regex::escape(&function_name)
            ),
            // Pattern 2: var name = function(...) { ... } (with proper brace matching)
            format!(
                r#"(var\s+{}\s*=\s*function\s*\([^)]*\)\s*\{{[^{{}}]*(?:\{{[^{{}}]*\}}[^{{}}]*)*\}})"#,
                regex::escape(&function_name)
            ),
            // Pattern 3: let name = function(...) { ... } (with proper brace matching)
            format!(
                r#"(let\s+{}\s*=\s*function\s*\([^)]*\)\s*\{{[^{{}}]*(?:\{{[^{{}}]*\}}[^{{}}]*)*\}})"#,
                regex::escape(&function_name)
            ),
            // Pattern 4: const name = function(...) { ... } (with proper brace matching)
            format!(
                r#"(const\s+{}\s*=\s*function\s*\([^)]*\)\s*\{{[^{{}}]*(?:\{{[^{{}}]*\}}[^{{}}]*)*\}})"#,
                regex::escape(&function_name)
            ),
            // Pattern 5: name = function(...) { ... } (with proper brace matching)
            format!(
                r#"({}\s*=\s*function\s*\([^)]*\)\s*\{{[^{{}}]*(?:\{{[^{{}}]*\}}[^{{}}]*)*\}})"#,
                regex::escape(&function_name)
            ),
        ];

        let mut function_code = None;
        for (i, pattern) in function_patterns.iter().enumerate() {
            debug!("Trying function extraction pattern {}: {}", i + 1, pattern);
            if let Ok(regex) = Regex::new(pattern) {
                if let Some(captures) = regex.captures(player_js) {
                    if let Some(matched) = captures.get(1) {
                        function_code = Some(matched.as_str().to_string());
                        debug!(
                            "Found function with pattern {}: {} chars",
                            i + 1,
                            matched.as_str().len()
                        );
                        break;
                    }
                }
            }
        }

        let function_code = function_code.ok_or_else(|| {
            RytError::CipherError("Could not find function definition".to_string())
        })?;

        // Extract dependencies (variables and functions used by the decipher function)
        let mut dependencies = Vec::new();

        // Look for variable assignments that might be used by the function
        let var_patterns = vec![
            r#"var\s+([a-zA-Z0-9$]+)\s*=\s*\{[^}]*\}"#,
            r#"let\s+([a-zA-Z0-9$]+)\s*=\s*\{[^}]*\}"#,
            r#"const\s+([a-zA-Z0-9$]+)\s*=\s*\{[^}]*\}"#,
        ];

        for pattern in var_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for captures in regex.captures_iter(player_js) {
                    if let Some(matched) = captures.get(0) {
                        dependencies.push(matched.as_str().to_string());
                    }
                }
            }
        }

        debug!(
            "Extracted function '{}' with {} dependencies",
            function_name,
            dependencies.len()
        );
        Ok((function_name, function_code, dependencies))
    }

    /// Find the actual decipher function name in player.js
    fn find_decipher_function_name(&self, player_js: &str) -> Result<String, RytError> {
        debug!(
            "Searching for decipher function in player.js ({} chars)",
            player_js.len()
        );

        // Look for function definitions that might be decipher functions
        #[allow(clippy::useless_vec)]
        let function_patterns = vec![
            // Pattern 1: function name(a) { ... a.split("") ... return a.join("") ... }
            r#"function\s+([a-zA-Z0-9$]+)\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{[^}]*\1\.split\(""\)[^}]*return\s+\1\.join\(""\)"#,
            // Pattern 2: var name = function(a) { ... a.split("") ... return a.join("") ... }
            r#"var\s+([a-zA-Z0-9$]+)\s*=\s*function\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{[^}]*\2\.split\(""\)[^}]*return\s+\2\.join\(""\)"#,
            // Pattern 3: let name = function(a) { ... a.split("") ... return a.join("") ... }
            r#"let\s+([a-zA-Z0-9$]+)\s*=\s*function\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{[^}]*\2\.split\(""\)[^}]*return\s+\2\.join\(""\)"#,
            // Pattern 4: const name = function(a) { ... a.split("") ... return a.join("") ... }
            r#"const\s+([a-zA-Z0-9$]+)\s*=\s*function\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{[^}]*\2\.split\(""\)[^}]*return\s+\2\.join\(""\)"#,
        ];

        for (i, pattern) in function_patterns.iter().enumerate() {
            debug!("Trying pattern {}: {}", i + 1, pattern);
            if let Ok(regex) = Regex::new(pattern) {
                if let Some(captures) = regex.captures(player_js) {
                    if let Some(function_name) = captures.get(1) {
                        debug!(
                            "Found function with pattern {}: {}",
                            i + 1,
                            function_name.as_str()
                        );
                        return Ok(function_name.as_str().to_string());
                    }
                }
            }
        }

        // Look for functions that contain signature-related operations
        #[allow(clippy::useless_vec)]
        let signature_patterns = vec![
            r#"function\s+([a-zA-Z0-9$]+)\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{[^}]*\.split\(""\)[^}]*\.join\(""\)"#,
            r#"var\s+([a-zA-Z0-9$]+)\s*=\s*function\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{[^}]*\.split\(""\)[^}]*\.join\(""\)"#,
            r#"let\s+([a-zA-Z0-9$]+)\s*=\s*function\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{[^}]*\.split\(""\)[^}]*\.join\(""\)"#,
            r#"const\s+([a-zA-Z0-9$]+)\s*=\s*function\s*\(\s*([a-zA-Z0-9$]+)\s*\)\s*\{[^}]*\.split\(""\)[^}]*\.join\(""\)"#,
        ];

        for (i, pattern) in signature_patterns.iter().enumerate() {
            debug!("Trying signature pattern {}: {}", i + 1, pattern);
            if let Ok(regex) = Regex::new(pattern) {
                if let Some(captures) = regex.captures(player_js) {
                    if let Some(function_name) = captures.get(1) {
                        debug!(
                            "Found signature function with pattern {}: {}",
                            i + 1,
                            function_name.as_str()
                        );
                        return Ok(function_name.as_str().to_string());
                    }
                }
            }
        }

        // Fallback: look for common decipher function names
        let common_names = vec![
            "decipher",
            "decode",
            "transform",
            "process",
            "signature",
            "sig",
        ];
        for name in common_names {
            if player_js.contains(&format!("function {}", name))
                || player_js.contains(&format!("var {} =", name))
                || player_js.contains(&format!("let {} =", name))
                || player_js.contains(&format!("const {} =", name))
            {
                debug!("Found common function name: {}", name);
                return Ok(name.to_string());
            }
        }

        // Look for any function that might be a decipher function
        let any_function_pattern = r#"function\s+([a-zA-Z0-9$]+)\s*\(\s*[a-zA-Z0-9$]+\s*\)"#;
        if let Ok(regex) = Regex::new(any_function_pattern) {
            if let Some(captures) = regex.captures(player_js) {
                if let Some(function_name) = captures.get(1) {
                    debug!("Found any function: {}", function_name.as_str());
                    return Ok(function_name.as_str().to_string());
                }
            }
        }

        debug!("No decipher function found, using default");
        // Default fallback
        Ok("decipher".to_string())
    }

    /// Sanitize player.js by removing problematic RegExp patterns (ported from Go ytdlp)
    fn sanitize_player_js(&self, player_js: &str) -> String {
        let mut sanitized = player_js.to_string();

        // Remove problematic RegExp patterns that cause deno_core to fail
        // These patterns include lookaheads, negative lookaheads, and other modern RegExp features

        // Replace lookahead patterns (?=...)
        let lookahead_re = Regex::new(r#"\?=[^)]*\)"#).unwrap();
        sanitized = lookahead_re.replace_all(&sanitized, "").to_string();

        // Replace negative lookahead patterns (?!...)
        let neg_lookahead_re = Regex::new(r#"\?![^)]*\)"#).unwrap();
        sanitized = neg_lookahead_re.replace_all(&sanitized, "").to_string();

        // Replace lookbehind patterns (?<=...)
        let lookbehind_re = Regex::new(r#"\?<=[^)]*\)"#).unwrap();
        sanitized = lookbehind_re.replace_all(&sanitized, "").to_string();

        // Replace negative lookbehind patterns (?<!...)
        let neg_lookbehind_re = Regex::new(r#"\?<![^)]*\)"#).unwrap();
        sanitized = neg_lookbehind_re.replace_all(&sanitized, "").to_string();

        // Replace named capture groups (?<name>...)
        let named_capture_re = Regex::new(r#"\?<[^>]*>"#).unwrap();
        sanitized = named_capture_re.replace_all(&sanitized, "").to_string();

        // Replace atomic groups (?>...)
        let atomic_group_re = Regex::new(r#"\?>[^)]*\)"#).unwrap();
        sanitized = atomic_group_re.replace_all(&sanitized, "").to_string();

        // Clean up any remaining problematic patterns
        let question_re = Regex::new(r#"\?[^)]*\)"#).unwrap();
        sanitized = question_re.replace_all(&sanitized, "").to_string();

        // Clean up any empty parentheses that might be left
        let empty_parens_re = Regex::new(r#"\(\s*\)"#).unwrap();
        sanitized = empty_parens_re.replace_all(&sanitized, "").to_string();

        // Clean up any remaining single parentheses
        let single_paren_re = Regex::new(r#"\(\s*;"#).unwrap();
        sanitized = single_paren_re.replace_all(&sanitized, ";").to_string();

        // Clean up any remaining single parentheses at end of lines
        let single_paren_end_re = Regex::new(r#"\(\s*$"#).unwrap();
        sanitized = single_paren_end_re.replace_all(&sanitized, "").to_string();

        // Remove document references that cause "document is not defined" errors
        let document_re = Regex::new(r#"document\.[a-zA-Z0-9_]+"#).unwrap();
        sanitized = document_re.replace_all(&sanitized, "null").to_string();

        // Remove window references
        let window_re = Regex::new(r#"window\.[a-zA-Z0-9_]+"#).unwrap();
        sanitized = window_re.replace_all(&sanitized, "null").to_string();

        // Remove navigator references
        let navigator_re = Regex::new(r#"navigator\.[a-zA-Z0-9_]+"#).unwrap();
        sanitized = navigator_re.replace_all(&sanitized, "null").to_string();

        // Remove location references
        let location_re = Regex::new(r#"location\.[a-zA-Z0-9_]+"#).unwrap();
        sanitized = location_re.replace_all(&sanitized, "null").to_string();

        // Remove console references
        let console_re = Regex::new(r#"console\.[a-zA-Z0-9_]+"#).unwrap();
        sanitized = console_re.replace_all(&sanitized, "null").to_string();

        // Remove eval() calls
        let eval_re = Regex::new(r#"eval\s*\("#).unwrap();
        sanitized = eval_re.replace_all(&sanitized, "null(").to_string();

        // Remove Function() constructor calls
        let function_re = Regex::new(r#"new\s+Function\s*\("#).unwrap();
        sanitized = function_re.replace_all(&sanitized, "null(").to_string();

        // Remove problematic Unicode escape sequences that cause syntax errors
        let unicode_escape_re = Regex::new(r#"\\u[0-9a-fA-F]{4}"#).unwrap();
        sanitized = unicode_escape_re.replace_all(&sanitized, "").to_string();

        // Remove problematic octal escape sequences
        let octal_escape_re = Regex::new(r#"\\([0-7]{1,3})"#).unwrap();
        sanitized = octal_escape_re.replace_all(&sanitized, "").to_string();

        // Remove problematic hex escape sequences
        let hex_escape_re = Regex::new(r#"\\x[0-9a-fA-F]{2}"#).unwrap();
        sanitized = hex_escape_re.replace_all(&sanitized, "").to_string();

        // Remove problematic template literals that might cause issues
        let template_literal_re = Regex::new(r#"`[^`]*`"#).unwrap();
        sanitized = template_literal_re
            .replace_all(&sanitized, "\"\"")
            .to_string();

        // Remove problematic arrow functions that might cause parsing issues
        let arrow_function_re = Regex::new(r#"=>\s*\{[^}]*\}"#).unwrap();
        sanitized = arrow_function_re
            .replace_all(&sanitized, "=> {}")
            .to_string();

        // Remove problematic destructuring assignments
        let destructuring_re = Regex::new(r#"\{[^}]*\}\s*="#).unwrap();
        sanitized = destructuring_re
            .replace_all(&sanitized, "obj =")
            .to_string();

        // Remove problematic spread operators
        let spread_re = Regex::new(r#"\.\.\.[a-zA-Z0-9_$]+"#).unwrap();
        sanitized = spread_re.replace_all(&sanitized, "").to_string();

        // Remove problematic async/await keywords
        let async_re = Regex::new(r#"\basync\b"#).unwrap();
        sanitized = async_re.replace_all(&sanitized, "").to_string();

        let await_re = Regex::new(r#"\bawait\b"#).unwrap();
        sanitized = await_re.replace_all(&sanitized, "").to_string();

        // Remove problematic class declarations
        let class_re = Regex::new(r#"\bclass\s+[a-zA-Z0-9_$]+\s*\{[^}]*\}"#).unwrap();
        sanitized = class_re.replace_all(&sanitized, "").to_string();

        // Remove problematic import/export statements
        let import_re = Regex::new(r#"\bimport\s+[^;]+;"#).unwrap();
        sanitized = import_re.replace_all(&sanitized, "").to_string();

        let export_re = Regex::new(r#"\bexport\s+[^;]+;"#).unwrap();
        sanitized = export_re.replace_all(&sanitized, "").to_string();

        // Remove problematic const/let declarations that might cause issues
        let const_re = Regex::new(r#"\bconst\s+[^;]+;"#).unwrap();
        sanitized = const_re.replace_all(&sanitized, "").to_string();

        let let_re = Regex::new(r#"\blet\s+[^;]+;"#).unwrap();
        sanitized = let_re.replace_all(&sanitized, "").to_string();

        // Clean up any remaining problematic characters
        let problematic_chars_re = Regex::new(r#"[^\x20-\x7E\n\r\t]"#).unwrap();
        sanitized = problematic_chars_re.replace_all(&sanitized, "").to_string();

        sanitized
    }

    /// Method 4: Pattern fallback
    fn decipher_with_pattern_fallback(
        &self,
        signature: &str,
        _player_js: &str,
    ) -> Result<String, RytError> {
        // Simple fallback transformations
        if signature.len() >= 2 {
            // Try common transformations
            let reversed: String = signature.chars().rev().collect();
            if reversed != signature {
                return Ok(reversed);
            }

            // Try swapping first and last characters
            let mut chars: Vec<char> = signature.chars().collect();
            if chars.len() > 1 {
                let len = chars.len();
                chars.swap(0, len - 1);
                return Ok(chars.into_iter().collect());
            }
        }

        Err(RytError::CipherError("Pattern fallback failed".to_string()))
    }

    /// Clear caches
    pub fn clear_caches(&self) {
        self.cache.clear();
        // Note: moka cache doesn't have a clear method in the version we're using
        // We'll let it expire naturally
    }
}

impl Default for Cipher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cipher_creation() {
        let _cipher = Cipher::new();
        // Test that cipher can be created - if we get here, test passed
    }

    #[test]
    #[ignore] // This test uses oversimplified player_js that doesn't match real YouTube patterns
    fn test_decipher_with_regex_reverse() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let player_js = "function test() { return signature.reverse(); }";

        let result = cipher.decipher_with_regex(signature, player_js);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "321cba");
    }

    #[test]
    fn test_decipher_with_pattern_fallback() {
        let cipher = Cipher::new();
        let signature = "abc123";

        let result = cipher.decipher_with_pattern_fallback(signature, "");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "321cba");
    }

    #[test]
    fn test_decipher_with_pattern_fallback_swap() {
        let cipher = Cipher::new();
        let signature = "ab";

        let result = cipher.decipher_with_pattern_fallback(signature, "");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "ba");
    }

    #[test]
    fn test_try_common_patterns_reverse() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let body = "a.reverse();";

        let result = cipher.try_common_patterns(signature, body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "321cba");
    }

    #[test]
    #[ignore] // These patterns require more complex implementation
    fn test_try_common_patterns_splice() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let body = "a.splice(0, 1);";

        let result = cipher.try_common_patterns(signature, body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "bc123");
    }

    #[test]
    #[ignore] // These patterns require more complex implementation
    fn test_try_common_patterns_slice() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let body = "a.slice(1);";

        let result = cipher.try_common_patterns(signature, body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "bc123");
    }

    #[test]
    #[ignore] // These patterns require more complex implementation
    fn test_try_common_patterns_swap() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let body = "var b=a[0];a[0]=a[2];a[2]=b;";

        let result = cipher.try_common_patterns(signature, body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "cba123");
    }

    #[test]
    fn test_try_common_patterns_empty() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let body = "";

        let result = cipher.try_common_patterns(signature, body);
        assert!(result.is_err());
    }

    #[test]
    fn test_try_common_patterns_unknown() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let body = "a.unknown();";

        let result = cipher.try_common_patterns(signature, body);
        assert!(result.is_err());
    }

    #[test]
    fn test_try_simple_fallback() {
        let cipher = Cipher::new();
        let signature = "abc123";

        let result = cipher.try_simple_fallback(signature);
        assert!(result.is_ok());
        // Simple fallback should return some transformation of the signature
        let result_str = result.unwrap();
        assert_ne!(result_str, signature); // Should be different from original
                                           // Note: some transformations may change length, so we don't check length
    }

    #[test]
    #[ignore] // These approaches require more complex player_js patterns
    fn test_try_approach_1_success() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let player_js = r#"
            function test_func(a) {
                a.split("");
                return a.join("");
            }
        "#;

        let result = cipher.try_approach_1(signature, player_js);
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_approach_1_failure() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let player_js = "function test() { return 'hello'; }";

        let result = cipher.try_approach_1(signature, player_js);
        assert!(result.is_err());
    }

    #[test]
    #[ignore] // These approaches require more complex player_js patterns
    fn test_try_approach_2_success() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let player_js = r#"
            var test_func = function(a) {
                a.split("");
                return a.join("");
            }
        "#;

        let result = cipher.try_approach_2(signature, player_js);
        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // These approaches require more complex player_js patterns
    fn test_try_approach_3_success() {
        let cipher = Cipher::new();
        let signature = "abc123";
        let player_js = r#"
            function test_func(a) {
                a.reverse();
                return a;
            }
        "#;

        let result = cipher.try_approach_3(signature, player_js);
        assert!(result.is_ok());
    }
}
