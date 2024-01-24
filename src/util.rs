use url::Url;

pub fn join_url(base_url: &str, path: &str) -> Result<Url, url::ParseError> {
    let parsed_base_url = Url::parse(base_url)?;
    let joined_url = parsed_base_url.join(path)?;
    Ok(joined_url)
}

// https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=5daf5180c98bd95564b93d66d3da0d3e
use core::marker::PhantomData;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde::Deserialize;

struct FieldsVisitor<V>(PhantomData<fn() -> V>);

impl<'de, V: Deserialize<'de>> Visitor<'de> for FieldsVisitor<V> {
    type Value = Vec<V>;

    fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("something")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut res: Vec<V> = Vec::with_capacity(map.size_hint().unwrap_or(0));

        while let Some((key, value)) = map.next_entry::<String, V>()? {
            // CQ: TODO don't hard code this
            if key == "selected_items" {
                res.push(value);
            }
        }

        Ok(res)
    }
}

pub fn deserialize_items<'de, D, V>(deserializer: D) -> Result<Vec<V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    deserializer.deserialize_map(FieldsVisitor(PhantomData))
}
