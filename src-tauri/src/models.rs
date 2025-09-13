use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RepoType {
    Branch,
    Encrypted,
    Decrypted,
}

