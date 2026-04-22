use serde::de::Deserialize;
use std::{collections::BTreeMap, ops::{Deref, DerefMut}, path::PathBuf};

use rbx_rsml::types::LanguageMode;

use crate::multibimap::MultiBiMap;

#[derive(Debug, Default)]
pub struct Aliases(pub BTreeMap<String, String>);

impl Deref for Aliases {
    type Target = BTreeMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Aliases {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Aliases {
    pub fn new<S: AsRef<str>>(contents: S) -> Self {
        Luaurc::new(contents).aliases
    }

    pub fn diff<'a>(
        &'a self,
        b: &'a Aliases
    ) -> impl Iterator<Item = &'a String>
    {
        let mut ia = self.iter();
        let mut ib = b.iter();

        let mut na = ia.next();
        let mut nb = ib.next();

        std::iter::from_fn(move || loop {
            match (na, nb) {
                (Some((ka, va)), Some((kb, vb))) => {
                    if ka == kb {
                        // same key
                        let out = if va == vb {
                            None
                        } else {
                            Some(ka)
                        };
                        na = ia.next();
                        nb = ib.next();
                        if out.is_some() {
                            return out;
                        }
                    } else if ka < kb {
                        // key only in a
                        let out = Some(ka);
                        na = ia.next();
                        return out;
                    } else {
                        // key only in b
                        let out = Some(kb);
                        nb = ib.next();
                        return out;
                    }
                }

                (Some((ka, _)), None) => {
                    na = ia.next();
                    return Some(ka);
                }

                (None, Some((kb, _))) => {
                    nb = ib.next();
                    return Some(kb);
                }
                (None, None) => return None,
            }
        })
    }

}

#[derive(Debug, Default)]
pub struct Dependants(MultiBiMap<String, PathBuf>);

impl Deref for Dependants {
    type Target = MultiBiMap<String, PathBuf>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Dependants {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Dependants {
    pub fn new() -> Self {
        Self(MultiBiMap::new())
    }
}


#[derive(Default, Debug)]
pub struct Luaurc {
    pub aliases: Aliases,
    pub dependants: Dependants,
    pub language_mode: LanguageMode,
}

impl<'de> Deserialize<'de> for Luaurc {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct LuaurcVisitor;

        impl<'de> Visitor<'de> for LuaurcVisitor {
            type Value = Luaurc;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a .luaurc configuration object")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut aliases = Aliases::default();
                let mut language_mode = LanguageMode::default();

                while let Some(key) = access.next_key::<String>()? {
                    match key.as_str() {
                        "aliases" => {
                            let map: BTreeMap<String, String> = access.next_value()?;
                            aliases = Aliases(map);
                        }
                        "languageMode" => {
                            let value: serde_json::Value = access.next_value()?;
                            language_mode = match value.as_str() {
                                Some("strict") => LanguageMode::Strict,
                                _ => LanguageMode::Nonstrict,
                            };
                        }
                        _ => {
                            let _: serde::de::IgnoredAny = access.next_value()?;
                        }
                    }
                }

                Ok(Luaurc {
                    aliases,
                    dependants: Dependants::new(),
                    language_mode,
                })
            }
        }

        deserializer.deserialize_map(LuaurcVisitor)
    }
}

impl Luaurc {
    pub fn new<S: AsRef<str>>(contents: S) -> Self {
        serde_json::from_str::<Luaurc>(contents.as_ref())
            .unwrap_or_else(|_| Luaurc::default())
    }
}
