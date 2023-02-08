// [[file:../../models.note::*imports][imports:1]]
use super::*;
use std::process::{Child, Command, Stdio};
// imports:1 ends here

// [[file:../../models.note::50a738a3][50a738a3]]
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
// 50a738a3 ends here

// [[file:../../models.note::6e72cbab][6e72cbab]]
use std::collections::HashMap;

pub struct Cmd {
    /// environment variables
    pub env_vars: HashMap<String, String>,
    /// The working directory
    pub wrk_dir: PathBuf,
    /// The cmdline to execute
    pub cmd: String,
}

impl Cmd {
    /// Return bash script.
    pub fn bash_script(&self) -> String {
        let wrk_dir = self.wrk_dir.shell_escape_lossy();
        let cmd = &self.cmd;
        let export_env: String = self
            .env_vars
            .iter()
            .map(|(var, value)| {
                let value = value.as_str();
                let value = value.shell_escape();
                format!("export {var}={value}\n")
            })
            .collect();

        format!(
            "#! /usr/bin/env bash
cd {wrk_dir}
{export_env}

{cmd}
"
        )
    }

    /// Generate bash script file in `path` ready to execute locally
    /// or remotely.
    pub fn generate_bash_script(&self, path: &Path) -> Result<()> {
        let script = self.bash_script();
        gut::fs::write_script_file(path, &script)?;
        Ok(())
    }
}
// 6e72cbab ends here
