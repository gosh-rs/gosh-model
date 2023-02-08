// [[file:../../models.note::*imports][imports:1]]
use super::*;
use std::process::{Child, Command, Stdio};
// imports:1 ends here

// [[file:../../models.note::6e72cbab][6e72cbab]]
use std::collections::HashMap;

/// Represents the command for local or remote execution
pub struct Cmd {
    /// environment variables
    pub env_vars: HashMap<String, PathBuf>,
    /// The working directory
    pub wrk_dir: PathBuf,
    /// The file to execute
    pub cmd: PathBuf,
}

impl Cmd {
    /// Return bash script.
    pub fn bash_script(&self) -> String {
        let wrk_dir = self.wrk_dir.shell_escape_lossy();
        let cmd = &self.cmd.shell_escape_lossy();
        let export_env: String = self
            .env_vars
            .iter()
            .map(|(var, value)| {
                let value = value.shell_escape_lossy();
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

// [[file:../../models.note::6d640b53][6d640b53]]
impl Cmd {
    // create Command for run `script`
    fn create_command(&self, script: &Path) -> std::process::Command {
        debug!("run script: {:?}", script);
        let mut command = Command::new(script);
        for (k, v) in &self.env_vars {
            trace!("env {k:?} = {v:?}");
        }
        command.current_dir(&self.wrk_dir).envs(&self.env_vars);
        command
    }

    // Run cmd with `input` as stdin, and returns output on success.
    pub fn run_with_input(&self, input: &str) -> Result<String> {
        let mut child = self
            .create_command(&self.cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to run script: {:?}", &self.cmd))?;

        let stdin = child.stdin.as_mut().context("Failed to open stdin")?;
        stdin.write_all(input.as_bytes()).context("Failed to write to stdin")?;

        let output = child.wait_with_output().context("Failed to read stdout")?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    // create child process
    pub fn create_child_process(&self) -> Result<Child> {
        let child = self
            .create_command(&self.cmd)
            .spawn()
            .with_context(|| format!("Failed to run main script: {:?}", &self.cmd))?;

        Ok(child)
    }
}
// 6d640b53 ends here

// [[file:../../models.note::5323ec2e][5323ec2e]]
impl BlackBoxModel {
    /// Create cmd for onetime execution. Interactive run is not
    /// handled here.
    fn create_onetime_cmd(&mut self, text: &str) -> Result<Cmd> {
        // TODO: prepare interact.sh
        let run_file = self.prepare_compute_env()?;

        let mut env_vars = vec![];
        let tpl_dir = self
            .tpl_file
            .parent()
            .ok_or(format_err!("bbm_tpl_file: invalid path: {:?}", self.tpl_file))?
            .to_owned();
        env_vars.push(("BBM_TPL_DIR".into(), tpl_dir));

        let job_dir = std::env::current_dir()?;
        env_vars.push(("BBM_JOB_DIR".into(), job_dir));

        let cmdline = format!("{}", run_file.display());
        debug!("submit cmdline: {}", cmdline);
        let wrk_dir = run_file.parent().unwrap().to_owned();

        let env_vars = env_vars.into_iter().collect();
        let cmd = run_file.to_owned();
        let cmd = Cmd { cmd, env_vars, wrk_dir };
        Ok(cmd)
    }

    /// Call run script with `text` as its standard input (stdin), and wait
    /// for process output (stdout)
    pub(super) fn submit_cmd(&mut self, text: &str) -> Result<String> {
        let mut cmd = self.create_onetime_cmd(text)?;

        // when in interactive mode, we call interact.sh script for output
        let out = if let Some(int_file) = &self.int_file {
            debug!("interactive mode enabled");
            // first time run: we store child proces to avoid being killed early
            if self.task.is_none() {
                let child = cmd.create_child_process()?;
                self.task = Task(child).into();
            }
            cmd.cmd = int_file.to_owned();
            cmd.run_with_input(text)?
        } else {
            cmd.run_with_input(text)?
        };

        Ok(out)
    }

    #[cfg(feature = "adhoc")]
    /// Create bash script pointing to `path` for onetime
    /// execution. Interactive run is not handled here.
    pub fn generate_bash_script(&mut self, mol: &Molecule, path: &Path) -> Result<()> {
        let txt = self.render_input(&mol)?;
        let cmd = self.create_onetime_cmd(&txt)?;
        cmd.generate_bash_script(path)?;
        Ok(())
    }
}
// 5323ec2e ends here
