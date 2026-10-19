use rand::RngCore;
use sha2::{Digest, Sha256};

pub fn generate_secure_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

// Simple hex encoding without external crate
mod hex {
    pub fn encode<T: AsRef<[u8]>>(data: T) -> String {
        data.as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_and_hashing() {
        let t1 = generate_secure_token();
        let t2 = generate_secure_token();
        assert_ne!(t1, t2);
        assert_eq!(t1.len(), 64);

        let h1 = hash_token(&t1);
        let h2 = hash_token(&t1);
        assert_eq!(h1, h2);
        assert_ne!(t1, h1);
    }
}
