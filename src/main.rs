mod normalize_path;
pub use normalize_path::NormalizePath;

mod rsml_to_model_json;
use rsml_to_model_json::rsml_to_model_json;

mod guarded_unwrap;

use clap::{Parser, Subcommand, crate_version};
use serde::Deserialize;

use std::{
    collections::HashSet,
    ffi::OsStr,
    fs,
    io::{Write, stdout},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam_channel::{RecvError, Sender, select};
use jod_thread::JoinHandle;
use memofs::{ReadDir, StdBackend, Vfs, VfsEvent};

use crate::{guarded_unwrap::GuardedUnwrap, luaurc::Aliases, multibimap::MultiBiMap};

pub mod luaurc;
use luaurc::Luaurc;

pub mod multibimap;
use multibimap::Ref;

#[derive(Deserialize)]
pub struct ModelJsonId {
    id: String,
}

#[derive(Debug)]
enum CreateFileDependencies<'a> {
    True(Option<&'a Path>),
    False,
}

fn model_json_is_rsml(path: &Path) -> bool {
    let contents = guarded_unwrap!(fs::read_to_string(path), return false);
    let model: ModelJsonId = guarded_unwrap!(serde_json::from_str(&contents), return false);

    model.id.ends_with(".rsml")
}

pub struct WatcherContext {
    pub vfs: Arc<Vfs>,
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub dependencies: MultiBiMap<PathBuf, PathBuf>,
    pub luaurc: Option<(PathBuf, Luaurc)>,
}

impl WatcherContext {
    fn handle_vfs_event(&mut self, event: VfsEvent) {
        self.vfs
            .commit_event(&event)
            .expect("Error applying VFS change");

        let path = match &event {
            VfsEvent::Create(path) | VfsEvent::Write(path) | VfsEvent::Remove(path) => {
                path.normalize()
            }

            _ => return,
        };

        if let Some(file_name) = path.file_name()
            && file_name.to_string_lossy().ends_with(".model.json")
        {
            return;
        }

        let is_rsml_ext = path.extension() == Some(OsStr::new("rsml"));

        if path.is_file() {
            if is_rsml_ext {
                self.dependencies.remove_by_left(path.clone());
                self.create_file(&path, CreateFileDependencies::True(None));

            // We have found our luaurc file.
            } else if let Some((luaurc_path, _)) = &self.luaurc
                && &path == luaurc_path
            {
                self.luaurc_update(luaurc_path.clone());
            }
        } else if path.is_dir() {
            self.recursive_scan(&path);

        // path no longer exists, remove it (the Remove event can't be relied upon).
        } else {
            if is_rsml_ext {
                let _ = fs::remove_file(&path);

                self.dependencies.remove_by_left(path.clone());

                if let Some((_, luaurc)) = self.luaurc.as_mut() {
                    luaurc.dependants.remove_by_right(path.clone());
                }

                let path_stem_str = path
                    .file_stem()
                    .map(|x| x.to_str().unwrap_or_default())
                    .unwrap_or_else(|| "");
                let _ = fs::remove_file(
                    path.join(format!("../{}.model.json", path_stem_str))
                        .normalize(),
                );

            // We can't decipher if the deleted path is a file or a directory,
            // so we treat it as if it were a directory. This should be fine as
            // we are only deleting dependencies whose path begins with this
            // deleted path - if a match is found then it is indeed a directory.
            } else {
                self.prune_dependencies(&path);
            }
        }
    }

    fn create_file(&mut self, path: &Path, create_dependencies: CreateFileDependencies) {
        let output_path = &{
            let mut output_path = self
                .output_dir
                .join(path.strip_prefix(&self.input_dir).unwrap());
            output_path.set_extension("model.json");
            output_path
        };

        let _ = fs::create_dir_all(&output_path.parent().unwrap());

        let model_json = rsml_to_model_json(&path, self);
        fs::write(output_path, model_json).unwrap();

        match create_dependencies {
            CreateFileDependencies::True(referent_path) => {
                let dependants = guarded_unwrap!(self.dependencies.get_by_right(path), return);

                if let Some(referent_path) = referent_path {
                    for dependant in dependants.clone() {
                        if referent_path == dependant.as_ref() {
                            continue;
                        }

                        self.create_file(&dependant, CreateFileDependencies::True(Some(path)));
                    }
                } else {
                    for dependant in dependants.clone() {
                        self.create_file(&dependant, CreateFileDependencies::True(Some(path)));
                    }
                };
            }

            CreateFileDependencies::False => (),
        };
    }

    fn luaurc_update(&mut self, luaurc_path: PathBuf) {
        let new_aliases =
            guarded_unwrap!(fs::read_to_string(&luaurc_path).map(Aliases::new), return);

        let (_, luaurc) = guarded_unwrap!(self.luaurc.take(), return);

        let diff = new_aliases.diff(&luaurc.aliases);

        let new_luaurc = Luaurc {
            aliases: Aliases(new_aliases.clone()),
            dependants: luaurc.dependants,
        };
        self.luaurc = Some((luaurc_path, new_luaurc));

        let luaurc = &mut self.luaurc.as_mut().unwrap().1;

        // Gets the files we need to update.
        let mut to_update: HashSet<Ref<PathBuf>> = HashSet::new();
        for key in diff {
            let dependants_for_alias =
                guarded_unwrap!(luaurc.dependants.get_by_left(key), continue);

            to_update.extend(dependants_for_alias.iter().cloned());
        }

        for path in to_update {
            self.create_file(path.0.as_path(), CreateFileDependencies::True(None));
        }
    }

    // Removes any dependencies which start with the specified path.
    fn prune_dependencies(&mut self, deleted_path: &Path) {
        let keys_to_prune_from_dependencies = self
            .dependencies
            .left_to_right
            .keys()
            .filter_map(|key| {
                if key.starts_with(deleted_path) {
                    Some(key.to_path_buf())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for key in keys_to_prune_from_dependencies {
            self.dependencies.remove_by_left(key);
        }

        if let Some((_, luaurc)) = self.luaurc.as_mut() {
            let keys_to_prune_from_luaurc_dependants = luaurc
                .dependants
                .right_to_left
                .keys()
                .filter_map(|key| {
                    if key.starts_with(deleted_path) {
                        Some(key.to_path_buf())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            for key in keys_to_prune_from_luaurc_dependants {
                luaurc.dependants.remove_by_right(key);
            }
        }
    }

    fn initialize(&mut self) {
        if let Some((luaurc_path, _)) = &self.luaurc {
            let _ = self.vfs.read(luaurc_path);
        };

        self.recursive_scan(&PathBuf::new());
    }

    fn recursive_scan(&mut self, offset_dir: &PathBuf) {
        if self.input_dir == self.output_dir {
            let offset_input_dir = &self.input_dir.join(offset_dir).normalize();

            self.recursive_scan_create_and_clean(self.vfs.read_dir(offset_input_dir));
        } else {
            let offset_input_dir = &self.input_dir.join(offset_dir).normalize();
            let offset_output_dir = &self.output_dir.join(offset_dir).normalize();

            self.recursive_scan_clean(self.vfs.read_dir(offset_output_dir));
            self.recursive_scan_create(self.vfs.read_dir(offset_input_dir));
        }
    }

    fn recursive_scan_create_and_clean(&mut self, dir: Result<ReadDir, std::io::Error>) {
        let dir = guarded_unwrap!(dir, return);

        for entry in dir {
            let path = guarded_unwrap!(&entry, continue).path();

            // Applies files for all of the directories descendants.
            if path.is_dir() {
                self.recursive_scan_create_and_clean(self.vfs.read_dir(path));
            } else if path.is_file() {
                // Creates the .model.json for the current .rsml file.
                if path.extension() == Some(OsStr::new("rsml")) {
                    self.create_file(&path.canonicalize().unwrap(), CreateFileDependencies::False);

                // Deletes .model.json file if it represents rsml as its considered stale.
                } else if path.to_string_lossy().ends_with(".model.json")
                    && model_json_is_rsml(path)
                {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }

    fn recursive_scan_create(&mut self, dir: Result<ReadDir, std::io::Error>) {
        let dir = guarded_unwrap!(dir, return);

        for entry in dir {
            let path = guarded_unwrap!(&entry, continue).path();

            // Applies files for all of the directories descendants.
            if path.is_dir() {
                self.recursive_scan_create(self.vfs.read_dir(path));

            // Creates the .model.json for the current .rsml file.
            } else if path.is_file() && path.extension() == Some(OsStr::new("rsml")) {
                self.create_file(&path.canonicalize().unwrap(), CreateFileDependencies::False);
            }
        }
    }

    fn recursive_scan_clean(&mut self, dir: Result<ReadDir, std::io::Error>) {
        let dir = guarded_unwrap!(dir, return);

        for entry in dir {
            let path = guarded_unwrap!(&entry, continue).path();

            // Applies files for all of the directories descendants.
            if path.is_dir() {
                self.recursive_scan_clean(self.vfs.read_dir(path));

            // Removes the .model.json file.
            } else if path.is_file()
                && path.to_string_lossy().ends_with(".model.json")
                && model_json_is_rsml(path)
            {
                let _ = fs::remove_file(path);
            }
        }
    }

    fn new(vfs: Vfs, input_dir: &Path, output_dir: &Path, luaurc_path: Option<&PathBuf>) -> Self {
        let input_dir = input_dir.canonicalize().unwrap();
        let output_dir = output_dir.canonicalize().unwrap();

        Self {
            vfs: Arc::new(vfs),
            input_dir,
            output_dir,
            dependencies: MultiBiMap::new(),
            luaurc: luaurc_path.map(|luaurc_path| {
                let read_to_string = fs::read_to_string(&luaurc_path);

                (
                    luaurc_path.clone(),
                    read_to_string.map(Luaurc::new).unwrap_or_default(),
                )
            }),
        }
    }
}

struct Watcher {
    #[allow(unused)]
    shutdown_sender: Sender<()>,

    #[allow(unused)]
    job_thread: JoinHandle<Result<(), RecvError>>,
}

impl Watcher {
    fn start(mut context: WatcherContext) -> Watcher {
        let start_time = Instant::now();

        let vfs_receiver = context.vfs.event_receiver();

        let (shutdown_sender, shutdown_receiver) = crossbeam_channel::bounded::<()>(1);

        let job_thread: JoinHandle<Result<(), RecvError>> = jod_thread::Builder::new()
            .name("ChangeProcessor thread".to_owned())
            .spawn(move || {
                loop {
                    select! {
                        recv(vfs_receiver) -> event => {
                            match event {
                                Ok(event) => {
                                    // Prevents events from the build step from polluting the watcher.
                                    // A bit of a band aid solution but it works.
                                    if start_time.elapsed() > Duration::from_millis(200) {
                                        context.handle_vfs_event(event)
                                    }
                                },
                                Err(err) => println!("err: {}", err)
                            }
                        },

                        recv(shutdown_receiver) -> _ => {
                            return Ok(());
                        }
                    }
                }
            })
            .expect("Could not start thread");

        Self {
            job_thread,
            shutdown_sender,
        }
    }
}

#[derive(Parser)]
#[command(name = "CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Watch {
        #[arg(value_enum, required = true)]
        input: PathBuf,

        #[arg(short, long)]
        output: Option<PathBuf>,

        #[arg(long = "luaurc")]
        luaurc_path: Option<PathBuf>,
    },

    Build {
        #[arg(value_enum, required = true)]
        input: PathBuf,

        #[arg(short, long)]
        output: Option<PathBuf>,

        #[arg(long = "luaurc")]
        luaurc_path: Option<PathBuf>,
    },

    Version,
}

trait FindFirstChild {
    fn find_first_child<F>(&self, predicate: F) -> Option<PathBuf>
    where
        F: FnMut(&PathBuf) -> bool;
}

impl FindFirstChild for Path {
    fn find_first_child<F>(&self, mut predicate: F) -> Option<PathBuf>
    where
        F: FnMut(&PathBuf) -> bool,
    {
        for entry in fs::read_dir(self).ok()? {
            let entry = entry.ok()?;
            let path = entry.path();
            if predicate(&path) {
                return Some(path);
            }
        }

        None
    }
}

impl FindFirstChild for PathBuf {
    fn find_first_child<F>(&self, predicate: F) -> Option<PathBuf>
    where
        F: FnMut(&PathBuf) -> bool,
    {
        self.as_path().find_first_child(predicate)
    }
}

impl FindFirstChild for &PathBuf {
    fn find_first_child<F>(&self, predicate: F) -> Option<PathBuf>
    where
        F: FnMut(&PathBuf) -> bool,
    {
        self.as_path().find_first_child(predicate)
    }
}

fn scan_for_luaurc(origin_dir: &PathBuf) -> Option<PathBuf> {
    origin_dir.find_first_child(|path| {
        if !path.is_file() {
            return false;
        };

        let prefix = path.file_prefix();

        prefix == Some(OsStr::new("luaurc"))
            || prefix == Some(OsStr::new(".luaurc"))
            || path.extension() == Some(OsStr::new("luaurc"))
    })
}

enum LuaurcStatus {
    // User specified luaurc path.
    Some(PathBuf),

    AutoSome(PathBuf),
    AutoNone,
}

impl LuaurcStatus {
    fn as_option(&self) -> Option<&PathBuf> {
        match self {
            Self::Some(luaurc_path) | Self::AutoSome(luaurc_path) => Some(luaurc_path),

            Self::AutoNone => None,
        }
    }
}

fn resolve_luaurc_path(
    input_dir: &PathBuf,
    luaurc_path: Option<PathBuf>,
) -> Result<LuaurcStatus, String> {
    if let Some(luaurc_path) = luaurc_path {
        match luaurc_path.canonicalize() {
            Ok(luaurc_path) => match luaurc_path.is_file() {
                true => return Ok(LuaurcStatus::Some(luaurc_path)),
                false => (),
            },
            Err(_) => {
                return Err(format!(
                    "ERROR: Could not find Luaurc at {:#?}",
                    luaurc_path.normalize()
                ));
            }
        }
    }

    match scan_for_luaurc(input_dir).or_else(|| scan_for_luaurc(&input_dir.join("../").normalize()))
    {
        Some(luaurc_path) => Ok(LuaurcStatus::AutoSome(luaurc_path)),
        None => Ok(LuaurcStatus::AutoNone),
    }
}

fn startup_message(
    prefix: &str,
    input_dir: &PathBuf,
    output_dir: Option<&PathBuf>,
    luaurc_path: &LuaurcStatus,
) -> String {
    let to_output_str = if let Some(output_dir) = output_dir {
        &format!(" to {:#?}", output_dir)
    } else {
        ""
    };

    match luaurc_path {
        LuaurcStatus::Some(luaurc_path) => format!(
            "Using Luaurc at {:#?}.\n{} {:#?}{}.",
            luaurc_path, prefix, input_dir, to_output_str
        ),

        LuaurcStatus::AutoSome(luaurc_path) => format!(
            "Using Luaurc automatically found at {:#?}.\n{} {:#?}{}.",
            luaurc_path, prefix, input_dir, to_output_str
        ),

        LuaurcStatus::AutoNone => format!(
            "No Luaurc was specified or automatically found.\n{} {:#?}{}.",
            prefix, input_dir, to_output_str
        ),
    }
}

fn canonicalize_input(path: &PathBuf) -> Result<PathBuf, String> {
    match path.canonicalize() {
        Ok(path) => match path.is_dir() {
            true => Ok(path),
            false => Err(format!(
                "ERROR: The specified input {:#?} is not a directory!",
                path.normalize()
            )),
        },
        Err(_) => Err(format!(
            "ERROR: The specified input directory {:#?} doesn't exist!",
            path.normalize()
        )),
    }
}

fn build(
    input: PathBuf,
    output: Option<PathBuf>,
    luaurc_path: Option<PathBuf>,
    label: &str,
) -> Option<WatcherContext> {
    let mut stdout = stdout();

    let input_dir = &match canonicalize_input(&input) {
        Ok(input_dir) => input_dir,
        Err(msg) => {
            let _ = writeln!(stdout, "{}", msg);
            return None;
        }
    };

    let output_dir = output.as_ref().unwrap_or(input_dir);

    let luaurc_status = match resolve_luaurc_path(input_dir, luaurc_path) {
        Ok(luaurc_status) => luaurc_status,

        Err(msg) => {
            let _ = writeln!(stdout, "{}", msg);
            return None;
        }
    };
    let luaurc_path = luaurc_status.as_option();

    let _ = fs::create_dir_all(&input_dir);
    let _ = fs::create_dir_all(&output_dir);

    let vfs = Vfs::new(StdBackend::new());
    let mut context = WatcherContext::new(vfs, &input_dir, &output_dir, luaurc_path);
    context.initialize();

    let _ = writeln!(
        stdout,
        "{}",
        startup_message(label, input_dir, output.as_ref(), &luaurc_status)
    );

    Some(context)
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Watch {
            input,
            output,
            luaurc_path,
        } => {
            let context = guarded_unwrap!(
                build(input, output, luaurc_path, "RSML CLI is now watching"),
                return
            );

            let _watcher = Watcher::start(context);

            std::thread::park();
        }

        Commands::Build {
            input,
            output,
            luaurc_path,
        } => {
            build(input, output, luaurc_path, "RSML CLI successfully built");
        }

        Commands::Version => {
            let mut stdout = stdout();
            let _ = writeln!(stdout, "RSML CLI Version: v{}", crate_version!());
        }
    }
}
