// header

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*header][header:1]]
//! Represents an universal blackbox (external) model defined by user scripts
//!
//! # Usage
//!
//! ```ignore
//! use gosh::models::*;
//! 
//! // initialize blackbox model from directory
//! let dir = "/share/apps/mopac/sp";
//! let bbm = BlackBox::from_dir(dir);
//! 
//! // use settings from current environment.
//! let bbm = BlackBox::from_env();
//! 
//! // calculate one molecule
//! let mp = bbm.compute(&mol)?;
//! 
//! // calculate a list of molecules
//! let mp_all = bbm.compute_bunch(&mols)?;
//! ```
// header:1 ends here

// imports

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*imports][imports:1]]
use serde::Deserialize;

use crate::common::*;
use crate::*;
use gchemol::Molecule;
// imports:1 ends here

// base

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*base][base:1]]
#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct BlackBox {
    /// Set the run script file for calculation.
    run_file: PathBuf,

    /// Set the template file for rendering molecule.
    tpl_file: PathBuf,

    /// Set the root directory for scratch files.
    scr_dir: Option<PathBuf>,

    // for internal uses
    #[serde(skip)]
    temp_dir: Option<TempDir>,
}

impl Default for BlackBox {
    fn default() -> Self {
        Self {
            run_file: "submit.sh".into(),
            tpl_file: "input.hbs".into(),
            scr_dir: None,
            temp_dir: None,
        }
    }
}
// base:1 ends here

// env

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*env][env:1]]
use dotenv;
use std::env;
use std::path::{Path, PathBuf};

/// Enter directory with environment variables from .env file
fn enter_dir_with_env(dir: &Path) -> Result<()> {
    info!("read dotenv vars from {}", dir.display());

    // change to directory
    // env::set_current_dir(&dir)?;

    // read environment variables
    dotenv::from_path(&dir.join(".env")).ok();
    Ok(())
}

impl BlackBox {
    /// Initialize from environment variables
    ///
    /// # Panic
    ///
    /// - Panic if the directory is inaccessible.
    ///
    fn from_dotenv(dir: &Path) -> Self {
        // read environment variables from .env config
        match enter_dir_with_env(dir) {
            Ok(_) => {}
            Err(e) => {
                warn!("no dotenv config found: {:?}", e);
            }
        }

        // construct from `BBM_*` environment variables
        for (key, value) in env::vars() {
            if key.starts_with("BBM") {
                info!("{}: {}", key, value);
            }
        }

        // canonicalize the file paths
        let mut bbm = BlackBox::from_env();
        bbm.run_file = dir.join(bbm.run_file);
        bbm.tpl_file = dir.join(bbm.tpl_file);

        bbm
    }

    /// Construct from environment variables
    fn from_env() -> Self {
        match envy::prefixed("BBM_").from_env::<BlackBox>() {
            Ok(bbm) => bbm,
            Err(error) => panic!("{:?}", error),
        }
    }
}
// env:1 ends here

// call

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*call][call:1]]
use tempfile::{tempdir, tempdir_in, TempDir};

impl BlackBox {
    /// Return a temporary directory under `BBM_SCR_ROOT` for safe calculation.
    fn new_scratch_directory(&self) -> Result<TempDir> {
        if let Some(ref scr_root) = self.scr_dir {
            info!("set scratch root directory as: {:?}", scr_root);
            Ok(tempdir_in(scr_root)?)
        } else {
            let tdir = tempdir()?;
            debug!("scratch root directory is not set, use the system default.");
            Ok(tdir)
        }
    }

    /// Call external script
    fn safe_call(&mut self, input: &str) -> Result<String> {
        debug!("run script file: {}", self.run_file.display());

        // re-use the same scratch directory for multi-step calculation, e.g.
        // optimization.
        let mut tdir_opt = self.temp_dir.take();

        let tdir = tdir_opt.get_or_insert_with(|| {
            self.new_scratch_directory()
                .map_err(|e| format_err!("Failed to create scratch directory:\n {:?}", e))
                .unwrap()
        });

        let ptdir = tdir.path();
        debug!("scratch dir: {}", ptdir.display());

        let cmdline = format!("{}", self.run_file.display());
        debug!("submit cmdline: {}", cmdline);

        let cdir = std::env::current_dir()?;
        let cmd_results = cmd!(&cmdline)
            .dir(ptdir)
            .env("BBM_WRK_DIR", cdir)
            .input(input)
            .read();

        // for re-using the scratch directory
        self.temp_dir = tdir_opt;

        Ok(cmd_results?)
    }
}
// call:1 ends here

// pub

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*pub][pub:1]]
impl BlackBox {
    /// Construct blackbox model under directory context.
    pub fn from_dir<P: AsRef<Path>>(dir: P) -> Self {
        Self::from_dotenv(dir.as_ref())
    }

    /// Render input using template
    pub fn render_input(&self, mol: &Molecule) -> Result<String> {
        // 1. load input template
        let template = gchemol::io::read_file(&self.tpl_file).map_err(|e| {
            error!("failed to load template");
            e
        })?;

        // 2. render input text with the template
        let txt = mol.render_with(&template)?;

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

    // keep scratch files for user inspection of failure.
    pub fn keep_scratch_files(self) {
        if let Some(tdir) = self.temp_dir {
            let path = tdir.into_path();
            println!("Directory for scratch files: {}", path.display());
        } else {
            warn!("No temp dir found.");
        }
    }
}
// pub:1 ends here

// pub/chemical model

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*pub/chemical%20model][pub/chemical model:1]]
use duct::cmd;

impl ChemicalModel for BlackBox {
    fn compute(&mut self, mol: &Molecule) -> Result<ModelProperties> {
        // 1. render input text with the template
        let txt = self.render_input(&mol)?;

        // 2. call external engine
        let output = self.safe_call(&txt)?;

        // 3. collect model properties
        let p: ModelProperties = output.parse()?;

        Ok(p)
    }

    fn compute_bunch(&mut self, mols: &[Molecule]) -> Result<Vec<ModelProperties>> {
        // 1. render input text with the template
        let txt = self.render_input_bunch(mols)?;

        // 2. call external engine
        let output = self.safe_call(&txt)?;

        // 3. collect model properties
        let all = ModelProperties::parse_all(&output)?;


        Ok(all)
    }
}
// pub/chemical model:1 ends here
