// [[file:../models.note::*header][header:1]]
//! Represents an universal blackbox (external) model defined by user scripts
//!
//! # Usage
//!
//! ```ignore
//! use gosh::models::*;
//! 
//! // initialize blackbox model from directory
//! let dir = "/share/apps/mopac/sp";
//! let bbm = BlackBoxModel::from_dir(dir)?;
//! 
//! // calculate one molecule
//! let mp = bbm.compute(&mol)?;
//! 
//! // calculate a list of molecules
//! let mp_all = bbm.compute_bunch(&mols)?;
//! ```
// header:1 ends here

// [[file:../models.note::c3765387][c3765387]]
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use super::*;
use gchemol::Molecule;
// c3765387 ends here

// [[file:../models.note::*base][base:1]]
pub struct BlackBoxModel {
    /// Set the run script file for calculation.
    run_file: PathBuf,

    /// Set the template file for rendering molecule.
    tpl_file: PathBuf,

    /// The script for interaction with the main process
    int_file: Option<PathBuf>,

    /// Set the root directory for scratch files.
    scr_dir: Option<PathBuf>,

    /// Job starting directory
    job_dir: Option<PathBuf>,

    // the field order matters
    // https://stackoverflow.com/questions/41053542/forcing-the-order-in-which-struct-fields-are-dropped
    task: Option<Task>,

    /// unique temporary working directory
    temp_dir: Option<TempDir>,

    /// Record the number of potential evalulations.
    ncalls: usize,
}
// base:1 ends here

// [[file:../models.note::045f62c4][045f62c4]]
// NOTE: There is no implementation of Drop for std::process::Child
/// A simple wrapper for killing child process on drop
struct Task(std::process::Child);

impl Drop for Task {
    // NOTE: There is no implementation of Drop for std::process::Child
    fn drop(&mut self) {
        info!("Task dropped. Kill external commands in session.");
        let child = &mut self.0;

        if let Ok(Some(x)) = child.try_wait() {
            info!("child process exited gracefully with status {x:?}.");
        } else {
            // inform child to exit gracefully
            if let Err(e) = send_signal_term(child.id()) {
                error!("Kill child process failure: {:?}", e);
            }
            std::thread::sleep(std::time::Duration::from_secs_f64(0.1));
            // wait a few seconds for child process to exit, or the scratch
            // directory will be removed immediately.
            if let Ok(None) = child.try_wait() {
                info!("Child process is still running, one second for clean up ...",);
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}

fn send_signal_term(pid: u32) -> Result<()> {
    use nix::sys::signal::{kill, Signal};

    let pid = nix::unistd::Pid::from_raw(pid as i32);
    let signal = Signal::SIGTERM;
    info!("Inform child process {} to exit by sending signal {:?}.", pid, signal);
    kill(pid, signal).with_context(|| format!("kill process {:?}", pid))?;

    Ok(())
}
// 045f62c4 ends here

// [[file:../models.note::6cc8ead1][6cc8ead1]]
mod env {
    use super::*;
    use tempfile::{tempdir, tempdir_in};

    /// Return a temporary directory under `BBM_SCR_ROOT` for safe calculation.
    fn new_scratch_directory(scr_root: Option<&Path>) -> Result<TempDir> {
        // create leading directories
        if let Some(d) = &scr_root {
            if !d.exists() {
                std::fs::create_dir_all(d).context("create scratch root dir")?;
            }
        }
        scr_root.map_or_else(
            || tempdir().context("create temp scratch dir"),
            |d| tempdir_in(d).with_context(|| format!("create temp scratch dir under {:?}", d)),
        )
    }

    impl BlackBoxModel {
        /// Create a temporary working directory and prepare running script
        pub(super) fn prepare_compute_env(&mut self) -> Result<PathBuf> {
            let run = "run";

            // create run script if it is not ready
            let runfile = if let Some(tdir) = &self.temp_dir {
                tdir.path().join(run)
            } else {
                let tdir = new_scratch_directory(self.scr_dir.as_deref())?;
                info!("BBM scratching directory: {:?}", tdir);

                // copy run script to work/scratch directory
                let dest = tdir.path().join(run);
                let txt = gut::fs::read_file(&self.run_file)?;
                gut::fs::write_script_file(&dest, &txt)?;

                // save temp dir for next execution
                self.temp_dir = tdir.into();
                dest.canonicalize()?
            };

            Ok(runfile)
        }

        pub(super) fn from_dotenv(dir: &Path) -> Result<Self> {
            // canonicalize the file paths
            let dir = dir
                .canonicalize()
                .with_context(|| format!("invalid template directory: {:?}", dir))?;

            // read environment variables from .env config if any
            let envfile = envfile::EnvFile::new(dir.join(".env")).unwrap();
            for (key, value) in &envfile.store {
                info!("found env var from {:?}: {}={}", &envfile.path, key, value);
            }

            let run_file = envfile.get("BBM_RUN_FILE").unwrap_or("submit.sh");
            let tpl_file = envfile.get("BBM_TPL_FILE").unwrap_or("input.hbs");
            let int_file_opt = envfile.get("BBM_INT_FILE");
            let bbm = BlackBoxModel {
                run_file: dir.join(run_file),
                tpl_file: dir.join(tpl_file),
                int_file: int_file_opt.map(|f| dir.join(f)),
                scr_dir: envfile.get("BBM_SCR_DIR").map(|x| x.into()),
                job_dir: std::env::current_dir()?.into(),
                temp_dir: None,
                task: None,
                ncalls: 0,
            };
            Ok(bbm)
        }

        // Construct from environment variables
        // 2020-09-05: it is dangerous if we have multiple BBMs in the sample process
        // fn from_env() -> Self {
        //     match envy::prefixed("BBM_").from_env::<BlackBoxModel>() {
        //         Ok(bbm) => bbm,
        //         Err(error) => panic!("{:?}", error),
        //     }
        // }
    }

    #[test]
    fn test_env() -> Result<()> {
        let d = new_scratch_directory(Some("/scratch/test".as_ref()))?;
        assert!(d.path().exists());
        let d = new_scratch_directory(None)?;
        assert!(d.path().exists());
        Ok(())
    }
}
// 6cc8ead1 ends here

// [[file:../models.note::50a738a3][50a738a3]]
mod cmd {
    use super::*;
    use std::process::{Child, Command, Stdio};

    impl BlackBoxModel {
        /// Call run script with `text` as its standard input (stdin), and wait
        /// for process output (stdout)
        pub(super) fn submit_cmd(&mut self, text: &str) -> Result<String> {
            // TODO: prepare interact.sh
            let run_file = self.prepare_compute_env()?;

            let tpl_dir = self
                .tpl_file
                .parent()
                .ok_or(format_err!("bbm_tpl_file: invalid path: {:?}", self.tpl_file))?;
            trace!("BBM_TPL_DIR: {:?}", tpl_dir);

            let cdir = std::env::current_dir()?;
            trace!("BBM_JOB_DIR: {:?}", cdir);

            let cmdline = format!("{}", run_file.display());
            debug!("submit cmdline: {}", cmdline);
            let tdir = run_file.parent().unwrap();

            // when in interactive mode, we call interact.sh script for output
            let out = if let Some(int_file) = &self.int_file {
                debug!("interactive mode enabled");
                // first time run: we store child proces to avoid being killed early
                if self.task.is_none() {
                    let child = process_create_normal(&run_file, tdir, tpl_dir, &cdir)?;
                    self.task = Task(child).into();
                }
                let child = process_create(&int_file, tdir, tpl_dir, &cdir)?;
                process_communicate(child, text)?
            } else {
                let child = process_create(&run_file, tdir, tpl_dir, &cdir)?;
                process_communicate(child, text)?
            };

            Ok(out)
        }
    }

    // create child process and capture stdin, stdout
    fn process_create(script: &Path, wrk_dir: &Path, tpl_dir: &Path, job_dir: &Path) -> Result<Child> {
        debug!("run script: {:?}", script);

        let child = Command::new(script)
            .current_dir(wrk_dir)
            .env("BBM_TPL_DIR", tpl_dir)
            .env("BBM_JOB_DIR", job_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to run script: {:?}", &script))?;

        Ok(child)
    }

    // create child process
    fn process_create_normal(script: &Path, wrk_dir: &Path, tpl_dir: &Path, job_dir: &Path) -> Result<Child> {
        debug!("run main script: {:?}", script);

        let child = Command::new(script)
            .current_dir(wrk_dir)
            .env("BBM_TPL_DIR", tpl_dir)
            .env("BBM_JOB_DIR", job_dir)
            .spawn()
            .with_context(|| format!("Failed to run main script: {:?}", &script))?;

        Ok(child)
    }

    // feed process stdin and get stdout
    fn process_communicate(mut child: std::process::Child, input: &str) -> Result<String> {
        {
            let stdin = child.stdin.as_mut().context("Failed to open stdin")?;
            stdin.write_all(input.as_bytes()).context("Failed to write to stdin")?;
        }

        let output = child.wait_with_output().context("Failed to read stdout")?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
// 50a738a3 ends here

// [[file:../models.note::360435b0][360435b0]]
impl BlackBoxModel {
    fn compute_normal(&mut self, mol: &Molecule) -> Result<Computed> {
        // 1. render input text with the template
        let txt = self.render_input(&mol)?;

        // 2. call external engine
        let output = self.submit_cmd(&txt)?;

        // 3. collect model properties
        let mp = output
            .parse()
            .with_context(|| format!("failed to parse computed results: {:?}", output))?;

        self.ncalls += 1;
        Ok(mp)
    }

    fn compute_normal_bunch(&mut self, mols: &[Molecule]) -> Result<Vec<Computed>> {
        // 1. render input text with the template
        let txt = self.render_input_bunch(mols)?;

        // 2. call external engine
        let output = self.submit_cmd(&txt)?;

        // 3. collect model properties
        let all = Computed::parse_all(&output)?;

        self.ncalls += 1;
        Ok(all)
    }
}
// 360435b0 ends here

// [[file:../models.note::*pub/input][pub/input:1]]
impl BlackBoxModel {
    /// Render input using template
    pub fn render_input(&self, mol: &Molecule) -> Result<String> {
        // check NaN values in positions
        for (i, a) in mol.atoms() {
            let p = a.position();
            if p.iter().any(|x| x.is_nan()) {
                error!("Invalid position of atom {}: {:?}", i, p);
                bail!("Molecule has invalid data in positions.");
            }
        }
        // render input text with external template file
        let txt = mol.render_with(&self.tpl_file)?;

        Ok(txt)
    }

    /// Render input using template in bunch mode.
    pub fn render_input_bunch(&self, mols: &[Molecule]) -> Result<String> {
        let mut txt = String::new();
        for mol in mols.iter() {
            let part = self.render_input(&mol)?;
            txt.push_str(&part);
        }

        Ok(txt)
    }
}
// pub/input:1 ends here

// [[file:../models.note::*pub/methods][pub/methods:1]]
impl BlackBoxModel {
    /// Construct BlackBoxModel model under directory context.
    pub fn from_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
        Self::from_dotenv(dir.as_ref()).context("Initialize BlackBoxModel failure.")
    }

    /// keep scratch files for user inspection of failure.
    pub fn keep_scratch_files(self) {
        if let Some(tdir) = self.temp_dir {
            let path = tdir.into_path();
            println!("Directory for scratch files: {}", path.display());
        } else {
            warn!("No temp dir found.");
        }
    }

    /// Return the number of potentail evaluations
    pub fn number_of_evaluations(&self) -> usize {
        self.ncalls
    }
}
// pub/methods:1 ends here

// [[file:../models.note::5ff4e3f1][5ff4e3f1]]
impl ChemicalModel for BlackBoxModel {
    fn compute(&mut self, mol: &Molecule) -> Result<Computed> {
        let mp = self.compute_normal(mol)?;

        // sanity checking: the associated structure should have the same number
        // of atoms
        debug_assert!({
            let n = mol.natoms();
            if let Some(pmol) = mp.get_molecule() {
                pmol.natoms() == n
            } else {
                true
            }
        });

        Ok(mp)
    }

    fn compute_bunch(&mut self, mols: &[Molecule]) -> Result<Vec<Computed>> {
        let all = self.compute_normal_bunch(mols)?;

        // one-to-one mapping
        debug_assert_eq!(mols.len(), all.len());
        Ok(all)
    }
}
// 5ff4e3f1 ends here

// [[file:../models.note::*test][test:1]]
#[test]
fn test_bbm() -> Result<()> {
    // setup two BBMs
    let bbm_vasp = "./tests/files/vasp-sp";
    let bbm_siesta = "./tests/files/siesta-sp";
    let vasp = BlackBoxModel::from_dir(bbm_vasp)?;
    let siesta = BlackBoxModel::from_dir(bbm_siesta)?;

    // VASP uses input.tera as the input template
    assert!(vasp.tpl_file.ends_with("input.tera"));
    // VASP uses input.hbs as the input template
    assert!(siesta.tpl_file.ends_with("input.hbs"));

    Ok(())
}
// test:1 ends here
