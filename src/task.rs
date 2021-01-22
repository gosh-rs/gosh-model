// [[file:../models.note::*imports][imports:1]]
use crate::core::*;
use crate::*;

use gut::prelude::*;

use gchemol::prelude::*;
use gchemol::Molecule;

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
// imports:1 ends here

// [[file:../models.note::*base][base:1]]
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use std::io::prelude::*;
use std::io::BufReader;
use std::io::LineWriter;

pub(crate) struct Task {
    child: Child,
    stream0: ChildStdin,
    stream1: std::io::Lines<BufReader<ChildStdout>>,
    wrk_dir: PathBuf,
    /// external script for suspending or resuming computation processes
    int_file: Option<PathBuf>,
}

impl Task {
    pub fn new(mut child: Child, wrk_dir: &Path) -> Self {
        let stream0 = child.stdin.take().unwrap();
        let stream1 = child.stdout.take().unwrap();
        Self {
            child,
            stream0,
            stream1: BufReader::new(stream1).lines(),
            wrk_dir: wrk_dir.to_owned(),
            int_file: None,
        }
    }

    /// Set interactive script
    pub fn interactive(mut self, int_file: &Path) -> Self {
        self.int_file = int_file.to_owned().into();
        self
    }
}
// base:1 ends here

// [[file:../models.note::*stop][stop:1]]
// FIXME: we should call interact.sh to resume paused processes, or vasp
// processes will be zombies
impl Drop for Task {
    fn drop(&mut self) {
        // resume processes before kill, or it will be blocked
        if let Some(int_file) = &self.int_file {
            if let Err(err) = interactive_resume(self.child.id(), int_file) {
                error!("found errors when resume processes: {:?}", err);
            }
        }

        info!("Force to kill child process: {}", self.child.id());
        if let Err(err) = self.child.kill() {
            dbg!(err);
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
        match self.child.try_wait() {
            Ok(Some(code)) => {
                info!("Done");
            }
            other => {
                dbg!(other);
            }
        }
    }
}
// stop:1 ends here

// [[file:../models.note::*interaction][interaction:1]]
fn interactive_resume(pid: u32, int_file: &Path) -> Result<String> {
    debug!("Resume process group {} using {:?}", pid, int_file);
    let out = duct::cmd!(int_file, "resume", &pid.to_string()).read()?;

    Ok(out)
}

fn interactive_suspend(pid: u32, int_file: &Path) -> Result<String> {
    debug!("Suspend process group {} using {:?}", pid, int_file);
    let out = duct::cmd!(int_file, "pause", &pid.to_string()).read()?;

    Ok(out)
}
// interaction:1 ends here

// [[file:../models.note::*compute & output][compute & output:1]]
impl Task {
    /// write scaled positions to VASP stdin
    fn input_positions(&mut self, mol: &Molecule) -> Result<()> {
        debug!("write scaled positions into stdin");
        let mut lines = mol
            .get_scaled_positions()
            .expect("lattice")
            .map(|[x, y, z]| format!("{:19.16} {:19.16} {:19.16}\n", x, y, z));

        for line in lines {
            self.stream0.write_all(line.as_bytes())?;
        }
        self.stream0.flush()?;

        Ok(())
    }

    fn compute_mol(&mut self, mol: &Molecule) -> Result<ModelProperties> {
        let mut text = String::new();
        while let Some(line) = self.stream1.next() {
            let line = line?;
            if line.starts_with("POSITIONS: reading from stdin") {
                let (energy, forces) = crate::vasp::stdout::parse_energy_and_forces(&text)?;
                let mut mp = ModelProperties::default();
                mp.set_energy(energy);
                mp.set_forces(forces);
                return Ok(mp);
            }
            writeln!(&mut text, "{}", line);
        }
        bail!("no model properties found!");
    }

    /// Caclculate model properties in an interactive fashion (with child
    /// process)
    ///
    /// # Parameters
    ///
    /// * mol: the molecule to be calculated
    /// * n: the total number of computations
    pub fn interact(&mut self, mol: &Molecule, n: usize) -> Result<ModelProperties> {
        debug!("interact with vasp process ...");

        // resume process before start interaction
        let pid = self.child.id();

        // it is not necessary to resume when just started
        if n != 0 {
            if let Some(int_file) = &self.int_file {
                let out = interactive_resume(pid, int_file)?;
                trace!("int_file stdout1: {:?}", out);
            }

            debug!("input positions");
            self.input_positions(mol)?;
        }
        debug!("recv outputs ...");

        let mp = self.compute_mol(mol)?;
        // suspend process after interaction
        if let Some(int_file) = &self.int_file {
            let out = interactive_suspend(pid, int_file)?;
            trace!("int_file stdout2: {:?}", out);
        }

        Ok(mp)
    }
}
// compute & output:1 ends here
