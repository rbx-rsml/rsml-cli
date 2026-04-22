use std::path::{Component, Path, PathBuf};

pub trait NormalizePath {
    fn normalize(&self) -> PathBuf;
}

impl<T: AsRef<Path>> NormalizePath for T {
    fn normalize(&self) -> PathBuf {
        let mut components = self.as_ref().components().peekable();
        let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
            components.next();
            PathBuf::from(c.as_os_str())
        } else {
            PathBuf::new()
        };

        for component in components {
            match component {
                Component::Prefix(..) => unreachable!(),
                Component::RootDir => {
                    ret.push(std::path::MAIN_SEPARATOR.to_string()); // RootDir handling
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    if ret.ends_with("..") {
                        ret.push("..");
                    } else {
                        let popped = ret.pop();
                        if !popped && !ret.has_root() {
                            ret.push("..");
                        }
                    }
                }
                Component::Normal(c) => {
                    ret.push(c);
                }
            }
        }
        ret
    }
}
