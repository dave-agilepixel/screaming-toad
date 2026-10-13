//! Secrets in the OS credential store (Windows Credential Manager, macOS
//! Keychain, Linux keyutils), shared by the desktop app and the CLI so
//! scheduled crawls can use the keys entered in the app.

const SERVICE: &str = "Screaming Toad";

fn entry(key: &str) -> anyhow::Result<keyring::Entry> {
    Ok(keyring::Entry::new(SERVICE, key)?)
}

pub fn get(key: &str) -> Option<String> {
    entry(key).ok()?.get_password().ok().filter(|s| !s.is_empty())
}

/// Save a secret; an empty value removes it.
pub fn set(key: &str, value: &str) -> anyhow::Result<()> {
    let e = entry(key)?;
    if value.trim().is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(x) => Err(x.into()),
        }
    } else {
        Ok(e.set_password(value.trim())?)
    }
}

impl crate::Secrets {
    /// Every known secret: TOAD_* environment variables first, then the OS
    /// credential store.
    pub fn load() -> Self {
        let mut s = Self::from_env();
        for k in crate::keys::ALL {
            if s.get(k).is_none() {
                if let Some(v) = get(k) {
                    s.set(k, &v);
                }
            }
        }
        s
    }
}
