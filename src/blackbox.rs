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
//! let bbm = BlackBox::from_dir(dir)?;
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

use crate::core::*;
use crate::*;
use gchemol::Molecule;
// imports:1 ends here

// [[file:../models.note::*base][base:1]]
#[derive(Debug)]
pub struct BlackBox {
    /// Set the run script file for calculation.
    run_file: PathBuf,

    /// Set the template file for rendering molecule.
    tpl_file: PathBuf,

    /// Set the root directory for scratch files.
    scr_dir: Option<PathBuf>,

    /// unique temporary working directory
    temp_dir: Option<TempDir>,
}
// base:1 ends here

// [[file:../models.note::*env][env:1]]
impl BlackBox {
    fn from_dotenv(dir: &Path) -> Result<Self> {
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
        let bbm = BlackBox {
            run_file: dir.join(run_file),
            tpl_file: dir.join(tpl_file),
            scr_dir: envfile.get("BBM_SCR_DIR").map(|x| x.into()),
            temp_dir: None,
        };
        Ok(bbm)
    }

    // Construct from environment variables
    // 2020-09-05: it is dangerous if we have multiple BBMs in the sample process
    // fn from_env() -> Self {
    //     match envy::prefixed("BBM_").from_env::<BlackBox>() {
    //         Ok(bbm) => bbm,
    //         Err(error) => panic!("{:?}", error),
    //     }
    // }
}
// env:1 ends here

// [[file:../models.note::*call][call:1]]
use tempfile::{tempdir, tempdir_in, TempDir};

impl BlackBox {
    /// Return a temporary directory under `BBM_SCR_ROOT` for safe calculation.
    fn new_scratch_directory(&self) -> Result<TempDir> {
        let tdir = if let Some(ref scr_root) = self.scr_dir {
            trace!("set scratch root directory as: {:?}", scr_root);
            tempdir_in(scr_root)?
        } else {
            let tdir = tempdir()?;
            debug!("scratch root directory is not set, use the system default.");
            tdir
        };
        info!("BBM scratching directory: {:?}", tdir);
        Ok(tdir)
    }

    /// Call external script
    fn safe_call(&mut self, input: &str) -> Result<String> {
        trace!("calling script file: {:?}", self.run_file);

        // re-use the same scratch directory for multi-step calculation, e.g.
        // optimization.
        let mut tdir_opt = self.temp_dir.take();

        let tdir = tdir_opt.get_or_insert_with(|| {
            self.new_scratch_directory()
                .with_context(|| format!("Failed to create scratch directory"))
                .unwrap()
        });
        let ptdir = tdir.path();

        trace!("scratch dir: {}", ptdir.display());

        let tpl_dir = self
            .tpl_file
            .parent()
            .ok_or(format_err!("bbm_tpl_file: invalid path: {:?}", self.tpl_file))?;

        trace!("BBM_TPL_DIR: {:?}", tpl_dir);
        let cdir = std::env::current_dir()?;
        trace!("BBM_JOB_DIR: {:?}", cdir);

        let cmdline = format!("{}", self.run_file.display());
        trace!("submit cmdline: {}", cmdline);
        let cmd = cmd!(&cmdline)
            .dir(ptdir)
            .env("BBM_TPL_DIR", tpl_dir)
            .env("BBM_JOB_DIR", cdir)
            .stdin_bytes(input);

        // for re-using the scratch directory
        self.temp_dir = tdir_opt;

        let stdout = cmd.read().context("BBM calling script failed.")?;

        Ok(stdout)
    }
}
// call:1 ends here

// [[file:../models.note::*pub][pub:1]]
impl BlackBox {
    /// Construct blackbox model under directory context.
    pub fn from_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
        Self::from_dotenv(dir.as_ref()).context("Initialize BlackBox model failed.")
    }

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

// [[file:../models.note::*pub/chemical model][pub/chemical model:1]]
use gut::cli::duct::cmd;

impl ChemicalModel for BlackBox {
    fn compute(&mut self, mol: &Molecule) -> Result<ModelProperties> {
        // 1. render input text with the template
        let txt = self.render_input(&mol).context("render input")?;

        // 2. call external engine
        let output = self.safe_call(&txt).context("call external model")?;

        // 3. collect model properties
        let p: ModelProperties = output.parse().context("parse results")?;

        // sanity checking: the associated structure should have the same number
        // of atoms
        debug_assert!({
            let n = mol.natoms();
            if let Some(pmol) = p.get_molecule() {
                pmol.natoms() == n
            } else {
                true
            }
        });

        Ok(p)
    }

    fn compute_bunch(&mut self, mols: &[Molecule]) -> Result<Vec<ModelProperties>> {
        // 1. render input text with the template
        let txt = self.render_input_bunch(mols)?;

        // 2. call external engine
        let output = self.safe_call(&txt)?;

        // 3. collect model properties
        let all = ModelProperties::parse_all(&output)?;

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
