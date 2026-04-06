use crate::librarian::error::Result;
use std::path::Path;

pub async fn blake3_hash_file(path: &Path) -> Result<String> {
    let bytes = tokio::fs::read(path).await?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn duplicate_payload_has_same_hash() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = dir.path().join("a.bin");
        let b = dir.path().join("b.bin");
        tokio::fs::write(&a, b"same payload")
            .await
            .expect("write a");
        tokio::fs::write(&b, b"same payload")
            .await
            .expect("write b");

        let h1 = blake3_hash_file(&a).await.expect("hash a");
        let h2 = blake3_hash_file(&b).await.expect("hash b");
        assert_eq!(h1, h2);
    }
}
