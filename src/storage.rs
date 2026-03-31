use std::path::PathBuf;

#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    /// Upload file bytes, return the public URL path
    async fn upload(&self, filename: &str, data: &[u8], content_type: &str) -> Result<String, String>;
    /// Delete a file by its URL path
    async fn delete(&self, url: &str) -> Result<(), String>;
}

// --- Local filesystem storage ---

pub struct LocalStorage {
    dir: PathBuf,
    url_prefix: String,
}

impl LocalStorage {
    pub fn new(dir: &str, url_prefix: &str) -> Self {
        let path = PathBuf::from(dir);
        std::fs::create_dir_all(&path).ok();
        Self {
            dir: path,
            url_prefix: url_prefix.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Storage for LocalStorage {
    async fn upload(&self, filename: &str, data: &[u8], _content_type: &str) -> Result<String, String> {
        let path = self.dir.join(filename);
        tokio::fs::write(&path, data)
            .await
            .map_err(|e| e.to_string())?;
        Ok(format!("{}/{}", self.url_prefix, filename))
    }

    async fn delete(&self, url: &str) -> Result<(), String> {
        let filename = url.rsplit('/').next().unwrap_or("");
        let path = self.dir.join(filename);
        tokio::fs::remove_file(&path).await.ok();
        Ok(())
    }
}

// --- S3 storage ---

#[cfg(feature = "s3")]
pub struct S3Storage {
    client: aws_sdk_s3::Client,
    bucket: String,
    url_prefix: String,
}

#[cfg(feature = "s3")]
impl S3Storage {
    pub async fn new(bucket: &str, endpoint: &str, region: &str, url_prefix: &str) -> Self {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region.to_string()))
            .endpoint_url(endpoint)
            .load()
            .await;
        let client = aws_sdk_s3::Client::from_conf(
            aws_sdk_s3::config::Builder::from(&config)
                .force_path_style(true) // Required for MinIO
                .build(),
        );
        Self {
            client,
            bucket: bucket.to_string(),
            url_prefix: url_prefix.to_string(),
        }
    }
}

#[cfg(feature = "s3")]
#[async_trait::async_trait]
impl Storage for S3Storage {
    async fn upload(&self, filename: &str, data: &[u8], content_type: &str) -> Result<String, String> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(filename)
            .body(data.to_vec().into())
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| format!("S3 upload error: {e:?}"))?;
        Ok(format!("{}/{}", self.url_prefix, filename))
    }

    async fn delete(&self, url: &str) -> Result<(), String> {
        let key = url.rsplit('/').next().unwrap_or("");
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
