mod rsml_to_model_json;
use guarded::guarded_unwrap;
use rsml_to_model_json::{rsml_to_model_json, StyleSheet};

use std::{ffi::OsStr, fs, path::{Path, PathBuf}, sync::Arc};

use crossbeam_channel::{select, RecvError, Sender};
use jod_thread::JoinHandle;
use memofs::{ReadDir, StdBackend, Vfs, VfsEvent};
use normalize_path::NormalizePath;

struct WatcherContext {
    vfs: Arc<Vfs>,
    input_dir: PathBuf,
    output_dir: PathBuf,
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

        // file no longer exists, remove it (the Remove event can't be relied upon).
        if !is_file && !path.is_dir() && path.extension() == Some(OsStr::new("rsml")) {
            self.remove_file(path);
        
        // applies utils from file.
        } else if is_file && path.starts_with(&self.input_dir) && path.extension() == Some(OsStr::new("rsml")) {
            self.create_file(path);
        }
    }

    fn create_file(&mut self, path: PathBuf) {
        let output_path = &{
            let mut output_path = self.output_dir.join(path.strip_prefix(&self.input_dir).unwrap());
            output_path.set_extension("model.json");
            output_path
        };

        fs::write(output_path, rsml_to_model_json(&path, &self.input_dir)).unwrap();
        let _ = fs::rename(output_path, output_path);
    }

    fn remove_file(&mut self, mut path: PathBuf) {
        path.set_extension("model.json");

        let _ = fs::remove_file(path);
    }

    fn create_initial(&mut self, dir: Result<ReadDir, std::io::Error>) {
        let dir = match dir {
            Ok(dir) => dir,
            Err(_) => return
        };
    
        for entry in dir {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue
            };
            let path = entry.path();
            
            // Applies files for all of the directories descendants.
            if path.is_dir() {
                self.create_initial(self.vfs.read_dir(path));
                
            // Creates the output for the current file.
            } else if path.is_file() && path.extension() == Some(OsStr::new("rsml")) {
                self.create_file(path.canonicalize().unwrap());
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
            output_dir
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
                println!("started");
                //log::trace!("ChangeProcessor thread started");

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
            .expect("Could not start ChangeProcessor thread");


        Self {
            job_thread,
            shutdown_sender
        }
    }
}

fn main() {
    let input_dir = &Path::new("./project/src");
    let output_dir = &Path::new("./project/src");

    let vfs = Vfs::new(StdBackend::new());

    let mut context = WatcherContext::new(vfs, input_dir, output_dir);

    let initial_dir = context.vfs.read_dir(input_dir);
    context.vfs.set_watch_enabled(false);

    context.create_initial(initial_dir);

    let _watcher = Watcher::start(context);
    
    std::thread::park();
}