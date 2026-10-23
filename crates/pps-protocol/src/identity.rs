//! Chave secreta de cada lado do túnel, guardada em disco.
//!
//! A chave pública derivada dela é a identidade no iroh. O PPS precisa de uma
//! chave estável (senão o ticket muda a cada reinício), e os apps também (na
//! fase 2, o PPS autoriza aparelhos pela chave pública).

use std::{fs, io::Write, path::Path};

use anyhow::Context;
use iroh::SecretKey;

/// Lê a chave de `path`, ou gera uma nova e salva lá se o arquivo não existir.
pub fn load_or_create(path: &Path) -> anyhow::Result<SecretKey> {
    match fs::read(path) {
        Ok(bytes) => {
            let bytes: [u8; 32] = bytes.as_slice().try_into().with_context(|| {
                format!(
                    "{} não tem 32 bytes; arquivo de chave corrompido?",
                    path.display()
                )
            })?;
            Ok(SecretKey::from_bytes(&bytes))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let key = SecretKey::generate();
            save(path, &key)?;
            Ok(key)
        }
        Err(err) => Err(err).with_context(|| format!("lendo {}", path.display())),
    }
}

fn save(path: &Path, key: &SecretKey) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("criando {}", path.display()))?;
    file.write_all(&key.to_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_then_reloads_same_key() {
        let dir = std::env::temp_dir().join(format!("pps-identity-{}", std::process::id()));
        let path = dir.join("chave");
        let _ = fs::remove_dir_all(&dir);

        let first = load_or_create(&path).unwrap();
        let second = load_or_create(&path).unwrap();
        assert_eq!(first.public(), second.public());

        fs::remove_dir_all(&dir).unwrap();
    }
}
