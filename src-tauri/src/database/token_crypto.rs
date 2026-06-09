use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use constant_time_eq::constant_time_eq;
use hmac::{Hmac, Mac};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use sha2::Sha256;
use std::path::Path;

use crate::error::AppError;

type HmacSha256 = Hmac<Sha256>;

const KEY_LEN: usize = 32;
const AEAD_NONCE_LEN: usize = 12;
const NONCE_LEN: usize = 16;
const LEGACY_PREFIX: &str = "ccs1";
const PREFIX: &str = "ccs2";

#[derive(Clone)]
pub(crate) struct TokenCipher {
    key: [u8; KEY_LEN],
}

impl TokenCipher {
    pub(crate) fn load_or_create(path: &Path) -> Result<Self, AppError> {
        if path.exists() {
            return Self::load_existing(path);
        }

        let mut key = [0u8; KEY_LEN];
        getrandom::getrandom(&mut key)
            .map_err(|e| AppError::Config(format!("Failed to generate auth secret key: {e}")))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
        }
        let encoded = URL_SAFE_NO_PAD.encode(key);
        if let Err(err) = write_private_key_file(path, &encoded) {
            if matches!(&err, AppError::Io { source, .. } if source.kind() == std::io::ErrorKind::AlreadyExists)
            {
                return Self::load_existing(path);
            }
            return Err(err);
        }
        set_private_permissions(path);
        Ok(Self { key })
    }

    fn load_existing(path: &Path) -> Result<Self, AppError> {
        let raw = std::fs::read_to_string(path).map_err(|e| AppError::io(path, e))?;
        set_private_permissions(path);
        let key = decode_key(raw.trim())?;
        Ok(Self { key })
    }

    pub(crate) fn ephemeral() -> Result<Self, AppError> {
        let mut key = [0u8; KEY_LEN];
        getrandom::getrandom(&mut key)
            .map_err(|e| AppError::Config(format!("Failed to generate auth test key: {e}")))?;
        Ok(Self { key })
    }

    pub(crate) fn encrypt(&self, plaintext: &str) -> Result<String, AppError> {
        let mut nonce = [0u8; AEAD_NONCE_LEN];
        getrandom::getrandom(&mut nonce)
            .map_err(|e| AppError::Config(format!("Failed to generate auth token nonce: {e}")))?;
        let key = aead_key(&self.key)?;
        let mut ciphertext = plaintext.as_bytes().to_vec();
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(PREFIX.as_bytes()),
            &mut ciphertext,
        )
        .map_err(|_| AppError::Config("Failed to encrypt managed auth token".to_string()))?;
        Ok(format!(
            "{PREFIX}:{}:{}",
            URL_SAFE_NO_PAD.encode(nonce),
            URL_SAFE_NO_PAD.encode(ciphertext),
        ))
    }

    pub(crate) fn decrypt_legacy_ok(&self, value: &str) -> Result<String, AppError> {
        if value.starts_with(&format!("{PREFIX}:")) {
            return self.decrypt_aead(value);
        }
        if value.starts_with(&format!("{LEGACY_PREFIX}:")) {
            return self.decrypt_legacy_v1(value);
        }
        Ok(value.to_string())
    }

    fn decrypt_aead(&self, value: &str) -> Result<String, AppError> {
        let mut parts = value.split(':');
        let prefix = parts.next();
        let nonce = parts.next();
        let ciphertext = parts.next();
        if prefix != Some(PREFIX) || parts.next().is_some() {
            return Err(AppError::Config(
                "Invalid encrypted auth token shape".to_string(),
            ));
        }
        let nonce = decode_part(nonce, "nonce")?;
        let nonce: [u8; AEAD_NONCE_LEN] = nonce
            .try_into()
            .map_err(|_| AppError::Config("Invalid encrypted auth token nonce".to_string()))?;
        let mut ciphertext = decode_part(ciphertext, "ciphertext")?;
        let key = aead_key(&self.key)?;
        let plaintext = key
            .open_in_place(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(PREFIX.as_bytes()),
                &mut ciphertext,
            )
            .map_err(|_| {
                AppError::Unauthorized(
                    "Encrypted managed auth token failed integrity check".to_string(),
                )
            })?;
        String::from_utf8(plaintext.to_vec())
            .map_err(|e| AppError::Config(format!("Encrypted auth token is not UTF-8: {e}")))
    }

    fn decrypt_legacy_v1(&self, value: &str) -> Result<String, AppError> {
        if !value.starts_with(&format!("{LEGACY_PREFIX}:")) {
            return Ok(value.to_string());
        }

        let mut parts = value.split(':');
        let prefix = parts.next();
        let nonce = parts.next();
        let ciphertext = parts.next();
        let expected_tag = parts.next();
        if prefix != Some(LEGACY_PREFIX) || parts.next().is_some() {
            return Err(AppError::Config(
                "Invalid encrypted auth token shape".to_string(),
            ));
        }
        let nonce = decode_part(nonce, "nonce")?;
        let ciphertext = decode_part(ciphertext, "ciphertext")?;
        let expected_tag = decode_part(expected_tag, "tag")?;
        if nonce.len() != NONCE_LEN {
            return Err(AppError::Config(
                "Invalid encrypted auth token nonce".to_string(),
            ));
        }

        let actual_tag = tag(&self.key, &nonce, &ciphertext)?;
        if !constant_time_eq(&actual_tag, &expected_tag) {
            return Err(AppError::Unauthorized(
                "Encrypted managed auth token failed integrity check".to_string(),
            ));
        }
        let plaintext = xor_with_keystream(&self.key, &nonce, &ciphertext)?;
        String::from_utf8(plaintext)
            .map_err(|e| AppError::Config(format!("Encrypted auth token is not UTF-8: {e}")))
    }
}

fn aead_key(key: &[u8; KEY_LEN]) -> Result<LessSafeKey, AppError> {
    let key = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| AppError::Config("Failed to initialize auth token cipher".to_string()))?;
    Ok(LessSafeKey::new(key))
}

fn decode_key(raw: &str) -> Result<[u8; KEY_LEN], AppError> {
    let decoded = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|e| AppError::Config(format!("Invalid auth secret key: {e}")))?;
    decoded
        .try_into()
        .map_err(|_| AppError::Config("Invalid auth secret key length".to_string()))
}

fn decode_part(part: Option<&str>, label: &str) -> Result<Vec<u8>, AppError> {
    URL_SAFE_NO_PAD
        .decode(part.unwrap_or_default())
        .map_err(|e| AppError::Config(format!("Invalid encrypted auth token {label}: {e}")))
}

fn tag(key: &[u8; KEY_LEN], nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, AppError> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| AppError::Config(format!("Failed to initialize auth token MAC: {e}")))?;
    mac.update(b"cc-switch-managed-auth-token-tag-v1");
    mac.update(nonce);
    mac.update(ciphertext);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn xor_with_keystream(
    key: &[u8; KEY_LEN],
    nonce: &[u8],
    input: &[u8],
) -> Result<Vec<u8>, AppError> {
    let mut output = Vec::with_capacity(input.len());
    let mut counter = 0u64;
    for chunk in input.chunks(32) {
        let mut mac = HmacSha256::new_from_slice(key).map_err(|e| {
            AppError::Config(format!("Failed to initialize auth token keystream: {e}"))
        })?;
        mac.update(b"cc-switch-managed-auth-token-stream-v1");
        mac.update(nonce);
        mac.update(&counter.to_be_bytes());
        let block = mac.finalize().into_bytes();
        output.extend(chunk.iter().zip(block.iter()).map(|(a, b)| a ^ b));
        counter = counter.saturating_add(1);
    }
    Ok(output)
}

#[cfg(unix)]
fn write_private_key_file(path: &Path, encoded: &str) -> Result<(), AppError> {
    use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| AppError::io(path, e))?;
    file.write_all(encoded.as_bytes())
        .map_err(|e| AppError::io(path, e))
}

#[cfg(not(unix))]
fn write_private_key_file(path: &Path, encoded: &str) -> Result<(), AppError> {
    std::fs::write(path, encoded).map_err(|e| AppError::io(path, e))
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(err) = std::fs::set_permissions(path, PermissionsExt::from_mode(0o600)) {
        log::warn!("Failed to set auth secret key permissions: {err}");
    }
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) {}

#[cfg(test)]
mod tests {
    use base64::Engine;

    use crate::error::AppError;

    use super::{TokenCipher, KEY_LEN};

    #[test]
    fn token_cipher_roundtrips_and_keeps_legacy_plaintext_readable() {
        let cipher = TokenCipher::ephemeral().expect("cipher");
        let encrypted = cipher.encrypt("secret-token").expect("encrypt");
        assert_ne!(encrypted, "secret-token");
        assert!(encrypted.starts_with("ccs2:"));
        assert_eq!(
            cipher.decrypt_legacy_ok(&encrypted).expect("decrypt"),
            "secret-token"
        );
        assert_eq!(
            cipher.decrypt_legacy_ok("legacy-token").expect("legacy"),
            "legacy-token"
        );
    }

    #[test]
    fn token_cipher_reads_legacy_v1_ciphertext() {
        let cipher = TokenCipher {
            key: [7u8; KEY_LEN],
        };
        let nonce = [11u8; super::NONCE_LEN];
        let ciphertext = super::xor_with_keystream(&cipher.key, &nonce, b"legacy-secret")
            .expect("legacy encrypt");
        let tag = super::tag(&cipher.key, &nonce, &ciphertext).expect("legacy tag");
        let encrypted = format!(
            "ccs1:{}:{}:{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(nonce),
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(ciphertext),
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(tag),
        );

        assert_eq!(
            cipher.decrypt_legacy_ok(&encrypted).expect("decrypt"),
            "legacy-secret"
        );
    }

    #[test]
    fn token_cipher_rejects_tampered_or_wrong_key_ciphertext() {
        let cipher = TokenCipher {
            key: [7u8; KEY_LEN],
        };
        let encrypted = cipher.encrypt("secret-token").expect("encrypt");
        let tampered = tamper_ciphertext(&encrypted);
        assert_ne!(tampered, encrypted);

        let err = cipher
            .decrypt_legacy_ok(&tampered)
            .expect_err("tampered ciphertext should fail");
        assert!(matches!(err, AppError::Unauthorized(_)));

        let wrong_key_cipher = TokenCipher {
            key: [8u8; KEY_LEN],
        };
        let err = wrong_key_cipher
            .decrypt_legacy_ok(&encrypted)
            .expect_err("wrong key should fail");
        assert!(matches!(err, AppError::Unauthorized(_)));
    }

    #[cfg(unix)]
    #[test]
    fn token_cipher_key_file_is_created_private() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let key_path = temp.path().join("managed-auth.key");
        let cipher = TokenCipher::load_or_create(&key_path).expect("load or create");
        let encrypted = cipher.encrypt("secret-token").expect("encrypt");
        assert_eq!(
            cipher.decrypt_legacy_ok(&encrypted).expect("decrypt"),
            "secret-token"
        );

        let mode = std::fs::metadata(&key_path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn token_cipher_existing_key_file_permissions_are_tightened() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let key_path = temp.path().join("managed-auth.key");
        let key = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([9u8; KEY_LEN]);
        std::fs::write(&key_path, key).expect("write loose key file");
        std::fs::set_permissions(&key_path, PermissionsExt::from_mode(0o644))
            .expect("loosen permissions");

        let cipher = TokenCipher::load_or_create(&key_path).expect("load existing key");
        let encrypted = cipher.encrypt("secret-token").expect("encrypt");
        assert_eq!(
            cipher.decrypt_legacy_ok(&encrypted).expect("decrypt"),
            "secret-token"
        );

        let mode = std::fs::metadata(&key_path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    fn tamper_ciphertext(value: &str) -> String {
        let mut parts = value.split(':');
        let prefix = parts.next().expect("prefix");
        let nonce = parts.next().expect("nonce");
        let ciphertext = parts.next().expect("ciphertext");
        assert_eq!(parts.next(), None);

        let mut ciphertext = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(ciphertext)
            .expect("decode ciphertext");
        ciphertext[0] ^= 0x01;

        format!(
            "{prefix}:{nonce}:{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(ciphertext)
        )
    }
}
