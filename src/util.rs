use std::env;

use url::Url;

use crate::AppError;

pub fn join_url(base_url: &str, path: &str) -> Result<Url, url::ParseError> {
    let parsed_base_url = Url::parse(base_url)?;
    let joined_url = parsed_base_url.join(path)?;
    Ok(joined_url)
}

pub fn get_suwayomi_url(path: &str) -> Option<String> {
    let suwayomi_url = env::var("SUWAYOMI_URL").ok()?;
    let parsed_url = Url::parse(&suwayomi_url).ok()?;
    let joined_url = parsed_url.join(path).ok()?;
    Some(joined_url.to_string())
}