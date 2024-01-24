use url::Url;

pub fn join_url(base_url: &str, path: &str) -> Result<Url, url::ParseError> {
    let parsed_base_url = Url::parse(base_url)?;
    let joined_url = parsed_base_url.join(path)?;
    Ok(joined_url)
}
