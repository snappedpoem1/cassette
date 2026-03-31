use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use blowfish::Blowfish;
use futures_util::StreamExt;
use std::path::Path;
use tokio::io::AsyncWriteExt;

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type BfCbcDec = cbc::Decryptor<Blowfish>;

pub fn md5_hex(payload: &[u8]) -> String {
    format!("{:x}", md5::compute(payload))
}

pub fn decrypt_aes256_cbc_pkcs7(
    encrypted: &[u8],
    key: &[u8],
    iv: &[u8],
) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err("aes-256 key must be exactly 32 bytes".to_string());
    }
    if iv.len() != 16 {
        return Err("cbc iv must be exactly 16 bytes".to_string());
    }

    let mut buffer = encrypted.to_vec();
    let decryptor = Aes256CbcDec::new_from_slices(key, iv)
        .map_err(|error| format!("failed to initialize decryptor: {error}"))?;
    let plaintext = decryptor
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|error| format!("decrypt failed: {error}"))?;

    Ok(plaintext.to_vec())
}

/// The well-known Deezer Blowfish master key used for track key derivation.
const DEEZER_BF_SECRET: &[u8; 16] = b"g4el58wc0zvf9na1";

/// Deezer Blowfish CBC IV — fixed for all tracks.
const DEEZER_BF_IV: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

/// Deezer encrypted chunk size (2 KiB).
const DEEZER_CHUNK_SIZE: usize = 2048;

/// Derive the per-track Blowfish key from a Deezer track ID.
///
/// Algorithm: MD5(track_id) → first 16 hex chars XOR last 16 XOR BF_SECRET.
pub fn deezer_track_key(track_id: &str) -> [u8; 16] {
    let hash = md5_hex(track_id.as_bytes());
    let hash_bytes = hash.as_bytes(); // 32 hex chars
    let mut key = [0u8; 16];
    for i in 0..16 {
        key[i] = hash_bytes[i] ^ hash_bytes[i + 16] ^ DEEZER_BF_SECRET[i];
    }
    key
}

/// Decrypt a full Deezer encrypted stream in-place.
///
/// Uses the stripe pattern: every 3rd chunk (0, 3, 6, …) is Blowfish-CBC
/// encrypted. All other chunks and any trailing partial chunk are plaintext.
pub fn decrypt_deezer_stream(data: &mut Vec<u8>, track_id: &str) {
    let key = deezer_track_key(track_id);
    let total = data.len();
    let full_chunks = total / DEEZER_CHUNK_SIZE;

    for chunk_idx in 0..full_chunks {
        if chunk_idx % 3 != 0 {
            continue;
        }
        let offset = chunk_idx * DEEZER_CHUNK_SIZE;
        let chunk = &mut data[offset..offset + DEEZER_CHUNK_SIZE];

        // Blowfish CBC decrypt with fixed IV, NoPadding (chunk is exact multiple of 8).
        if let Ok(decryptor) = BfCbcDec::new_from_slices(&key, &DEEZER_BF_IV) {
            // decrypt_padded_mut with NoPadding — we know the chunk is exactly 2048 bytes
            // which is a multiple of Blowfish's 8-byte block size.
            use aes::cipher::block_padding::NoPadding;
            let _ = decryptor.decrypt_padded_mut::<NoPadding>(chunk);
        }
    }
    // Trailing partial chunk (< 2048 bytes) is always plaintext — no action needed.
}

pub async fn stream_decrypt_deezer_to_file(
    response: reqwest::Response,
    destination: &Path,
    track_id: &str,
) -> Result<u64, String> {
    let key = deezer_track_key(track_id);
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| error.to_string())?;
    let mut pending = Vec::<u8>::new();
    let mut chunk_index = 0usize;
    let mut written = 0u64;

    while let Some(next) = stream.next().await {
        let bytes = next.map_err(|error| error.to_string())?;
        pending.extend_from_slice(&bytes);

        while pending.len() >= DEEZER_CHUNK_SIZE {
            let mut chunk = pending.drain(..DEEZER_CHUNK_SIZE).collect::<Vec<_>>();
            if chunk_index % 3 == 0 {
                if let Ok(decryptor) = BfCbcDec::new_from_slices(&key, &DEEZER_BF_IV) {
                    use aes::cipher::block_padding::NoPadding;
                    let _ = decryptor.decrypt_padded_mut::<NoPadding>(&mut chunk);
                }
            }
            file.write_all(&chunk)
                .await
                .map_err(|error| error.to_string())?;
            written += chunk.len() as u64;
            chunk_index += 1;
        }
    }

    if !pending.is_empty() {
        file.write_all(&pending)
            .await
            .map_err(|error| error.to_string())?;
        written += pending.len() as u64;
    }

    file.flush().await.map_err(|error| error.to_string())?;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};

    type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

    #[test]
    fn md5_hex_matches_known_vector() {
        assert_eq!(md5_hex(b"cassette"), "097353568a36b5b81e66f5ec72df53f5");
    }

    #[test]
    fn decrypt_round_trip() {
        let key = [7_u8; 32];
        let iv = [9_u8; 16];
        let plaintext = b"cassette-director-crypto";

        let mut buffer = vec![0_u8; plaintext.len() + 32];
        buffer[..plaintext.len()].copy_from_slice(plaintext);
        let ciphertext = Aes256CbcEnc::new_from_slices(&key, &iv)
            .expect("init encryptor")
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
            .expect("encrypt")
            .to_vec();

        let decrypted = decrypt_aes256_cbc_pkcs7(&ciphertext, &key, &iv).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }
}
