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

// [[file:../models.note::*imports][imports:1]]
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use crate::core::*;
use crate::*;
use gchemol::Molecule;
// imports:1 ends here

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

    // the order is matters
    // https://stackoverflow.com/questions/41053542/forcing-the-order-in-which-struct-fields-are-dropped
    task: Option<crate::task::Task>,

    /// unique temporary working directory
    temp_dir: Option<TempDir>,

    /// Record the number of potential evalulations.
    ncalls: usize,
}
// base:1 ends here

// [[file:../models.note::*env][env:1]]
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
        /// Create a temporary working directory and prepare running scripts
        pub(super) fn prepare_compute_env(&mut self) -> Result<PathBuf> {
            use std::os::unix::fs::PermissionsExt;

            let tdir = new_scratch_directory(self.scr_dir.as_deref())?;
            info!("BBM scratching directory: {:?}", tdir);

            // copy run file to work/scratch directory, and make sure it is
            // executable
            let dest = tdir.path().join("run");
            std::fs::copy(&self.run_file, &dest)
                .with_context(|| format!("copy {:?} to {:?}", &self.run_file, &dest))?;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755)).context("chmod +x")?;

            self.temp_dir = tdir.into();

            Ok(dest.canonicalize()?)
        }

        pub(super) fn from_dotenv(dir: &Path) -> Result<Self> {
            // canonicalize the file paths
            let dir = dir
                .canonicalize()
                .with_context(|| format!("invalid template directory: {:?}", dir))?;

            // read environment variables from .env config if any
            let mut envfile = envfile::EnvFile::new(dir.join(".env")).unwrap();
            for (key, value) in &envfile.store {
                info!("found env var from {:?}: {}={}", &envfile.path, key, value);
            }

            let run_file = envfile.get("BBM_RUN_FILE").unwrap_or("submit.sh");
            let tpl_file = envfile.get("BBM_TPL_FILE").unwrap_or("input.hbs");
            let int_file_opt = envfile.get("BBM_INT_FILE");
            let mut bbm = BlackBoxModel {
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
// env:1 ends here

// [[file:../models.note::*cmd][cmd:1]]
mod cmd {
    use super::*;
    use std::process::{Child, Command, Stdio};

    impl BlackBoxModel {
        pub(super) fn is_server_started(&self) -> bool {
            self.task.is_some()
        }

        /// Call run script with `text` as its stdin
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

            // write POSCAR for interactive VASP calculation
            // FIXME: looks dirty
            let out = if let Some(int_file) = &self.int_file {
                info!("interactive mode enabled");
                // FIXME: to refactor out
                gut::fs::write_to_file(&tdir.join("POSCAR"), text)?;

                let child = run_script(&run_file, tdir, tpl_dir, &cdir)?;
                self.task = crate::task::Task::new(child, tdir).interactive(int_file).into();

                // return an empty string
                String::new()
            } else {
                call_with_input(&run_file, text, tdir, tpl_dir, &cdir)?
            };

            Ok(out)
        }
    }

    fn run_script(script: &Path, wrk_dir: &Path, tpl_dir: &Path, job_dir: &Path) -> Result<Child> {
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

    /// Call external script and get its output (stdout)
    fn call_with_input(script: &Path, input: &str, wrk_dir: &Path, tpl_dir: &Path, job_dir: &Path) -> Result<String> {
        use std::io::Write;

        let mut child = run_script(script, wrk_dir, tpl_dir, job_dir)?;
        {
            let stdin = child.stdin.as_mut().context("Failed to open stdin")?;
            stdin.write_all(input.as_bytes()).context("Failed to write to stdin")?;
        }

        let output = child.wait_with_output().context("Failed to read stdout")?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
// cmd:1 ends here

// [[file:../models.note::*compute][compute:1]]
impl BlackBoxModel {
    fn compute_normal(&mut self, mol: &Molecule) -> Result<ModelProperties> {
        // 1. render input text with the template
        let txt = self.render_input(&mol)?;

        // 2. call external engine
        let output = self.submit_cmd(&txt)?;

        // 3. collect model properties
        let mp = output.parse().context("parse results")?;

        Ok(mp)
    }

    fn compute_normal_bunch(&mut self, mols: &[Molecule]) -> Result<Vec<ModelProperties>> {
        // 1. render input text with the template
        let txt = self.render_input_bunch(mols)?;

        // 2. call external engine
        let output = self.submit_cmd(&txt)?;

        // 3. collect model properties
        let all = ModelProperties::parse_all(&output)?;

        Ok(all)
    }

    // TODO: make it more general
    fn compute_interactive(&mut self, mol: &Molecule) -> Result<ModelProperties> {
        info!("Enter interactive vasp calculation mode ...");
        let first_run = !self.is_server_started();
        // start child process as a server
        if first_run {
            debug!("first time run");
            let text = self.render_input(mol)?;
            self.submit_cmd(&text)?;
        }
        assert!(self.is_server_started());

        let mp = self.task.as_mut().unwrap().interact(mol, self.ncalls)?;

        Ok(mp)
    }
}
// compute:1 ends here

// [[file:../models.note::*pub/input][pub/input:1]]
impl BlackBoxModel {
    /// Render input using template
    pub fn render_input(&self, mol: &Molecule) -> Result<String> {
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

// [[file:../models.note::*pub/chemical model][pub/chemical model:1]]
impl ChemicalModel for BlackBoxModel {
    fn compute(&mut self, mol: &Molecule) -> Result<ModelProperties> {
        let mp = if self.int_file.is_some() {
            self.compute_interactive(mol)?
        } else {
            self.compute_normal(mol)?
        };
        self.ncalls += 1;

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

    fn compute_bunch(&mut self, mols: &[Molecule]) -> Result<Vec<ModelProperties>> {
        let all = if self.int_file.is_some() {
            error!("bunch calculation in interactive mode is not supported yet!");
            unimplemented!()
        } else {
            self.compute_normal_bunch(mols)?
        };
        self.ncalls += 1;

        // one-to-one mapping
        debug_assert_eq!(mols.len(), all.len());
        Ok(all)
    }
}
// pub/chemical model:1 ends here

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
