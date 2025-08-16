mod normalize_path;
pub use normalize_path::NormalizePath;

mod rsml_to_model_json;
use rsml_to_model_json::rsml_to_model_json;

mod guarded_unwrap;

use multimap::MultiMap;
use clap::{Parser, Subcommand, crate_version};
use serde::Deserialize;

use std::{ffi::OsStr, fs, io::{stdout, Write}, path::{Path, PathBuf}, sync::Arc};

use crossbeam_channel::{select, RecvError, Sender};
use jod_thread::JoinHandle;
use memofs::{ReadDir, StdBackend, Vfs, VfsEvent};

use crate::guarded_unwrap::GuardedUnwrap;

#[derive(Deserialize)]
pub struct ModelJsonId {
    id: String
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
    pub dependencies: MultiMap<PathBuf, PathBuf>
}

impl WatcherContext {
    fn handle_vfs_event(&mut self, event: VfsEvent) {
        self.vfs
            .commit_event(&event)
            .expect("Error applying VFS change");

        let path = match &event {
            VfsEvent::Create(path) | VfsEvent::Write(path) | VfsEvent::Remove(path) => {
                path.normalize()
            },

            _ => return
        };

        let is_file = path.is_file();
        
        // We found a change for an .rsml file.
        if is_file && path.starts_with(&self.input_dir) && path.extension() == Some(OsStr::new("rsml")) {
            self.create_file(&path);

        // file no longer exists, remove it (the Remove event can't be relied upon).
        } else if !is_file && !path.is_dir() {
            let _ = fs::remove_file(&path);

            let path_stem_str = path.file_stem()
                .map(|x| x.to_str().unwrap_or_default())
                .unwrap_or_else(|| "");
            let _ = fs::remove_file(path.join(format!("../{}.model.json", path_stem_str)).normalize());
        }
    }

    fn create_file(&mut self, path: &Path) {
        let output_path = &{
            println!("{:#?}", path);
            let mut output_path = self.output_dir.join(path.strip_prefix(&self.input_dir).unwrap());
            output_path.set_extension("model.json");
            output_path
        };

        let _ = fs::create_dir_all(&output_path.parent().unwrap());

        fs::write(output_path, rsml_to_model_json(&path, self)).unwrap();
        let _ = fs::rename(output_path, output_path);

        // Rebuilds dependants.
        // TODO: find a way to avoid cloning here.
        let dependants = guarded_unwrap!(self.dependencies.get_vec(path), return);
        for dependant in dependants.clone() {
            self.create_file(&dependant);
        }
    }

    fn initialize(&mut self) {
        if self.input_dir == self.output_dir {
            self.initialize_create_and_clean(self.vfs.read_dir(&self.input_dir));

        } else {
            self.initialize_clean(self.vfs.read_dir(&self.output_dir));
            self.initialize_create(self.vfs.read_dir(&self.input_dir));
        }
    }

    fn initialize_create_and_clean(&mut self, dir: Result<ReadDir, std::io::Error>) {
        let dir = guarded_unwrap!(dir, return);
    
        for entry in dir {
            let path = guarded_unwrap!(&entry, continue).path();
            
            // Applies files for all of the directories descendants.
            if path.is_dir() {
                self.initialize_create_and_clean(self.vfs.read_dir(path));
                
            } else if path.is_file() {
                // Creates the .model.json for the current .rsml file.
                if path.extension() == Some(OsStr::new("rsml")) {
                    self.create_file(&path.canonicalize().unwrap());

                // Deletes .model.json file if it represents rsml as its considered stale.
                } else if 
                    path.to_string_lossy().ends_with(".model.json") && 
                    model_json_is_rsml(path)
                {
                   let _ = fs::remove_file(path);
                }
            }
        }
    }

    fn initialize_create(&mut self, dir: Result<ReadDir, std::io::Error>) {
        let dir = guarded_unwrap!(dir, return);

        for entry in dir {
            let path = guarded_unwrap!(&entry, continue).path();
            
            // Applies files for all of the directories descendants.
            if path.is_dir() {
                self.initialize_create(self.vfs.read_dir(path));
                
            // Creates the .model.json for the current .rsml file.
            } else if path.is_file() && path.extension() == Some(OsStr::new("rsml")) {
                self.create_file(&path.canonicalize().unwrap());
            }
        }
    }

    fn initialize_clean(&mut self, dir: Result<ReadDir, std::io::Error>) {
        let dir = guarded_unwrap!(dir, return);

        for entry in dir {
            let path = guarded_unwrap!(&entry, continue).path();
            
            // Applies files for all of the directories descendants.
            if path.is_dir() {
                self.initialize_clean(self.vfs.read_dir(path));
                
            // Creates the .model.json for the current .rsml file.
            } else if path.is_file() &&
                path.to_string_lossy().ends_with(".model.json") && 
                model_json_is_rsml(path)
            {
                let _ = fs::remove_file(path);
            }
        }
    }

    fn new(
        vfs: Vfs, 
        input_dir: &Path,
        output_dir: &Path
    ) -> Self {
        let input_dir = input_dir.canonicalize().unwrap();
        let output_dir = output_dir.canonicalize().unwrap();

        Self {
            vfs: Arc::new(vfs),
            input_dir,
            output_dir,
            dependencies: MultiMap::new()
        }
    }
}

struct Watcher {
    shutdown_sender: Sender<()>,

    #[allow(unused)]
    job_thread: JoinHandle<Result<(), RecvError>>,
}

impl Watcher {
    fn start(mut context: WatcherContext) -> Watcher {
        let vfs_receiver = context.vfs.event_receiver();

        let (shutdown_sender, shutdown_receiver) = crossbeam_channel::bounded::<()>(1);

        let job_thread: JoinHandle<Result<(), RecvError>> = jod_thread::Builder::new()
            .name("ChangeProcessor thread".to_owned())
            .spawn(move || {
                loop {
                    select! {
                        recv(vfs_receiver) -> event => {
                            match event {
                                Ok(event) => context.handle_vfs_event(event),
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
            shutdown_sender
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
    },

    Build {
        #[arg(value_enum, required = true)]
        input: PathBuf,

        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    Version
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Watch { input, output } => {
            let input_dir = &input;
            let output_dir = output.as_ref().unwrap_or(input_dir);
            
            let _ = fs::create_dir_all(&input_dir);
            let _ = fs::create_dir_all(&output_dir);

            let vfs = Vfs::new(StdBackend::new());
            let mut context = WatcherContext::new(vfs, &input_dir, &output_dir);
            context.initialize();

            let mut stdout = stdout();
            let _ = writeln!(stdout, "RSML CLI is now watching {:#?}.", context.input_dir);

            let _watcher = Watcher::start(context);
    
            std::thread::park();
        },

        Commands::Build { input, output } => {
            let input_dir = &input;
            let output_dir = output.as_ref()
                .unwrap_or(input_dir);
            
            let _ = fs::create_dir_all(&input_dir);
            let _ = fs::create_dir_all(&output_dir);

            let vfs = Vfs::new(StdBackend::new());
            let mut context = WatcherContext::new(vfs, &input_dir, &output_dir);
            context.initialize();

            let mut stdout = stdout();

            if output.is_some() {
                let _ = writeln!(stdout, "RSML CLI successfully built {:#?} to {:#?}.", context.input_dir, context.output_dir);
            } else {
                let _ = writeln!(stdout, "RSML CLI successfully built {:#?}.", context.input_dir);
            }
        },

        Commands::Version => {
            let mut stdout = stdout();
            let _ = writeln!(
                stdout,
                "RSML CLI Version: v{}", crate_version!()
            );
        }
    }
}