use serde::de::{Deserialize};
use std::{collections::{BTreeMap}, ops::{Deref, DerefMut}, path::PathBuf};

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

impl<'de> Deserialize<'de> for Aliases {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct AliasesVisitor;

        impl<'de> Visitor<'de> for AliasesVisitor {
            type Value = Aliases;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with an 'aliases' key")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut aliases = None;
                while let Some(key) = access.next_key::<String>()? {
                    if key == "aliases" {
                        aliases = Some(access.next_value()?);
                    } else {
                        let _: serde::de::IgnoredAny = access.next_value()?;
                    }
                }
                let aliases = aliases.ok_or_else(|| serde::de::Error::missing_field("aliases"))?;
                Ok(Aliases(aliases))
            }
        }

        deserializer.deserialize_map(AliasesVisitor)
    }
}

impl Aliases {
    pub fn new<S: AsRef<str>>(contents: S) -> Self {
        serde_json::from_str::<Aliases>(contents.as_ref())
            .unwrap_or_else(|_| Aliases::default())
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
    pub dependants: Dependants
}

impl Luaurc {
    pub fn new<S: AsRef<str>>(contents: S) -> Self {
        Self {
            aliases: Aliases::new(contents),
            dependants: Dependants::new()
        }
    }
}