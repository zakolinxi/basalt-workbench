use keyring::Entry;

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.basalt.workbench";

fn entry(provider_id: &str) -> AppResult<Entry> {
    Entry::new(SERVICE, provider_id).map_err(AppError::Keyring)
}

pub fn set_api_key(provider_id: &str, key: &str) -> AppResult<()> {
    if key.trim().is_empty() {
        return delete_api_key(provider_id);
    }
    entry(provider_id)?.set_password(key.trim()).map_err(AppError::Keyring)
}

pub fn get_api_key(provider_id: &str) -> Option<String> {
    entry(provider_id)
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|k| !k.trim().is_empty())
}

pub fn has_api_key(provider_id: &str) -> bool {
    get_api_key(provider_id).is_some()
}

pub fn delete_api_key(provider_id: &str) -> AppResult<()> {
    match entry(provider_id)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(AppError::Keyring(err)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn service_name_is_stable() {
        assert_eq!(super::SERVICE, "com.basalt.workbench");
    }
}
