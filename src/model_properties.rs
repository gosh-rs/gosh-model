// imports

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*imports][imports:1]]
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use crate::core::*;

use gchemol::prelude::*;
use gchemol::Molecule;
// imports:1 ends here

// base

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*base][base:1]]
const MODEL_PROPERTIES_FORMAT_VERSION: &str = "0.1";

/// The computed results by external application
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelProperties {
    energy: Option<f64>,
    forces: Option<Vec<[f64; 3]>>,
    dipole: Option<[f64; 3]>,
    #[serde(skip_deserializing, skip_serializing)]
    molecule: Option<Molecule>,
    #[serde(skip_deserializing, skip_serializing)]
    force_constants: Option<Vec<[f64; 3]>>,
}
// base:1 ends here

// display/parse

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*display/parse][display/parse:1]]
impl ModelProperties {
    /// Parse mulitple entries of ModelProperties from string slice
    pub fn parse_all(output: &str) -> Result<Vec<ModelProperties>> {
        parse_model_results(output)
    }

    /// Return true if there is no useful properties
    pub fn is_empty(&self) -> bool {
        //self.energy.is_none() && self.forces.is_none() && self.molecule.is_none()
        self.energy.is_none() && self.forces.is_none()
    }
}

impl fmt::Display for ModelProperties {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut txt = format!(
            "@model_properties_format_version {}\n",
            MODEL_PROPERTIES_FORMAT_VERSION
        );

        // structure
        if let Some(mol) = &self.molecule {
            txt.push_str("@structure\n");
            let coords = mol.format_as("text/pxyz").expect("formatted molecule");
            txt.push_str(&coords);
        }
        // energy
        if let Some(energy) = &self.energy {
            txt.push_str("@energy\n");
            txt.push_str(&format!("{:-20.12E}\n", energy));
        }
        // forces
        if let Some(forces) = &self.forces {
            txt.push_str("@forces\n");
            for [fx, fy, fz] in forces {
                let line = format!("{:-20.12E} {:-20.12E} {:-20.12E}\n", fx, fy, fz);
                txt.push_str(&line);
            }
        }

        // dipole moments
        if let Some(d) = &self.dipole {
            txt.push_str("@dipole\n");
            let line = format!("{:-20.12E} {:-20.12E} {:-20.12E}\n", d[0], d[1], d[2]);
            txt.push_str(&line);
        }

        write!(f, "{}", txt)
    }
}

impl FromStr for ModelProperties {
    type Err = gut::prelude::Error;

    fn from_str(s: &str) -> Result<Self> {
        let all = parse_model_results(s)?;

        let n = all.len();
        if n == 0 {
            bail!("no valid results found!");
        }

        Ok(all[n - 1].clone())
    }
}

// parse a single entry of ModelProperties
fn parse_model_results_single(part: &[&str]) -> Result<ModelProperties> {
    // collect records as header separated lines
    // blank lines are ignored
    let mut records: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut header = None;
    for line in part {
        let line = line.trim();
        if line.starts_with("@") {
            header = line.split_whitespace().next();
        } else {
            if let Some(k) = header {
                records.entry(k).or_insert(vec![]).push(line);
            }
        }
    }

    // parse record values
    if records.len() < 1 {
        warn!("collected no results! Please check if the stream is clean.");
    }

    let mut results = ModelProperties::default();
    for (k, lines) in records {
        match k {
            "@energy" => {
                assert_eq!(1, lines.len(), "expect one line containing energy");
                let energy = lines[0].trim().parse()?;
                results.energy = Some(energy);
            }
            "@forces" => {
                let mut forces: Vec<[f64; 3]> = vec![];
                for line in lines {
                    let parts: Vec<_> = line.split_whitespace().collect();
                    if parts.len() != 3 {
                        bail!("expect xyz forces: {}", line);
                    }
                    let fx = parts[0].parse()?;
                    let fy = parts[1].parse()?;
                    let fz = parts[2].parse()?;
                    forces.push([fx, fy, fz]);
                }

                results.forces = Some(forces);
            }
            "@structure" => {
                let mut s = lines.join("\n");
                s.push_str("\n\n");
                let mol = Molecule::from_str(&s, "text/pxyz")?;
                results.molecule = Some(mol);
            }
            "@dipole" => {
                assert_eq!(1, lines.len(), "expect one line containing dipole moment");
                let parts: Vec<_> = lines[0].split_whitespace().collect();
                let fx = parts[0].parse()?;
                let fy = parts[1].parse()?;
                let fz = parts[2].parse()?;
                results.dipole = Some([fx, fy, fz]);
            }
            _ => {
                warn!("ignored record: {:?}", k);
            }
        }
    }

    Ok(results)
}

fn parse_model_results(stream: &str) -> Result<Vec<ModelProperties>> {
    if stream.trim().is_empty() {
        bail!("Attemp to parse empty string!");
    }

    // ignore commenting lines or blank lines
    let lines: Vec<_> = stream
        .lines()
        .filter(|l| {
            let l = l.trim();
            !l.starts_with("#") && !l.is_empty()
        })
        .collect();

    let parts = lines[1..].split(|l| l.starts_with("@model_properties_format_version"));

    let mut all_results = vec![];
    for part in parts {
        // collect records as header separated lines
        // blank lines are ignored
        let mp = parse_model_results_single(part)?;
        all_results.push(mp);
    }

    Ok(all_results)
}
// display/parse:1 ends here

use gchemol::Atom;
use gchemol::Lattice;

impl ModelProperties {
    /// Set item energy.
    pub fn set_energy(&mut self, e: f64) {
        self.energy = Some(e);
    }

    /// Set item forces.
    pub fn set_forces(&mut self, f: Vec<[f64; 3]>) {
        self.forces = Some(f);
    }

    /// Set item dipole.
    pub fn set_dipole(&mut self, d: [f64; 3]) {
        self.dipole = Some(d);
    }

    /// Set item Molecule.
    pub fn set_molecule(&mut self, m: Molecule) {
        self.molecule = Some(m);
    }

    /// Set item force constants.
    pub fn set_force_constants(&mut self, fc: Vec<[f64; 3]>) {
        self.force_constants = Some(fc);
    }

    /// Get energy component.
    pub fn get_energy(&self) -> Option<f64> {
        self.energy
    }

    /// Get dipole moment component.
    pub fn get_dipole(&self) -> Option<[f64; 3]> {
        self.dipole
    }

    /// Get forces component.
    pub fn get_forces(&self) -> Option<&Vec<[f64; 3]>> {
        self.forces.as_ref()
    }

    /// Get molecule structure.
    pub fn get_molecule(&self) -> Option<&Molecule> {
        self.molecule.as_ref()
    }

    /// Get force constants component.
    pub fn get_force_constants(&self) -> Option<&Vec<[f64; 3]>> {
        self.force_constants.as_ref()
    }

    #[cfg(feature = "adhoc")]
    /// Set molecule structure.
    ///
    /// # Parameters
    ///
    /// * atoms: a list of symbol and position pairs
    ///
    /// * cell: three Lattice vectors array
    ///
    /// * scaled: if input positions are in scaled coordinates
    pub fn set_structure<A, C>(&mut self, atoms: A, cell: Option<C>, scaled: bool)
    where
        A: IntoIterator,
        A::Item: Into<Atom>,
        C: Into<[[f64; 3]; 3]>,
    {
        let mut mol = Molecule::from_atoms(atoms);

        if let Some(lat) = cell.map(|x| Lattice::new(x.into())) {
            mol.set_lattice(lat);
            if scaled {
                let positions: Vec<_> = mol.positions().collect();
                mol.set_scaled_positions(positions);
            }
        }

        self.molecule = Some(mol);
    }
}

// test

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*test][test:1]]
#[test]
fn test_model_parse_results() {
    use approx::*;

    use serde_json;

    let txt = gchemol::io::read_file("tests/files/sample.txt").unwrap();
    let r: ModelProperties = txt.parse().expect("model results");

    // serializing
    let serialized = serde_json::to_string(&r).unwrap();
    // and deserializing
    let _: ModelProperties = serde_json::from_str(&serialized).unwrap();

    // reformat
    let txt = format!("{}", r);

    // parse again
    let r: ModelProperties = txt.parse().expect("model results");

    assert!(&r.molecule.is_some());
    let ref mol = r.molecule.unwrap();
    assert_eq!(3, mol.natoms());
    let e = r.energy.expect("model result: energy");
    assert_relative_eq!(-0.329336, e, epsilon = 1e-4);
}
// test:1 ends here
