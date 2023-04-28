// [[file:../models.note::b456354a][b456354a]]
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use super::*;

use gchemol::prelude::*;
use gchemol::Atom;
use gchemol::Lattice;
use gchemol::Molecule;
// b456354a ends here

// [[file:../models.note::7de724a0][7de724a0]]
const MODEL_PROPERTIES_FORMAT_VERSION: &str = "0.1";

/// The computed model properties by external application
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Computed {
    energy: Option<f64>,
    forces: Option<Vec<[f64; 3]>>,
    dipole: Option<[f64; 3]>,
    #[serde(skip_deserializing, skip_serializing)]
    molecule: Option<Molecule>,
    #[serde(skip_deserializing, skip_serializing)]
    force_constants: Option<Vec<[f64; 3]>>,
}
// 7de724a0 ends here

// [[file:../models.note::3b493716][3b493716]]
#[derive(Debug, Clone)]
struct Header {
    name: String,
    unit_factor: f64,
}

impl FromStr for Header {
    type Err = gut::prelude::Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.starts_with("@") {
            let mut unit_factor = 1.0;
            let parts = &s[1..].split_whitespace().collect_vec();
            let name = parts[0].into();
            if parts.len() > 1 {
                for p in &parts[1..] {
                    if let Some((k, v)) = p.split_once('=') {
                        if k == "unit_factor" {
                            unit_factor = v.parse::<f64>()?;
                        }
                    }
                }
            }
            Ok(Self { name, unit_factor })
        } else {
            bail!("invalid model properties section header: {}", s);
        }
    }
}

#[test]
fn test_header() {
    let s = "@forces ";
    let h: Header = s.parse().unwrap();
    assert_eq!(h.name, "forces");
    assert_eq!(h.unit_factor, 1.0);

    let s = "@forces unit_factor=1";
    let h: Header = s.parse().unwrap();
    assert_eq!(h.unit_factor, 1.0);

    let s = "@forces unit_factor=-1 test=2";
    let h: Header = s.parse().unwrap();
    assert_eq!(h.unit_factor, -1.0);
}
// 3b493716 ends here

// [[file:../models.note::37f15603][37f15603]]
impl Computed {
    /// Parse mulitple entries of Computed from string slice
    pub fn parse_all(output: &str) -> Result<Vec<Computed>> {
        parse_model_results(output)
    }

    /// Return true if there is no useful properties
    pub fn is_empty(&self) -> bool {
        //self.energy.is_none() && self.forces.is_none() && self.molecule.is_none()
        self.energy.is_none() && self.forces.is_none()
    }
}

impl fmt::Display for Computed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut txt = format!("@model_properties_format_version {}\n", MODEL_PROPERTIES_FORMAT_VERSION);

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

impl FromStr for Computed {
    type Err = gut::prelude::Error;

    fn from_str(s: &str) -> Result<Self> {
        let all = parse_model_results(s)?;

        let n = all.len();
        if n == 0 {
            bail!("no valid results found from:\n {s:?}!");
        }

        Ok(all[n - 1].clone())
    }
}

// parse a single entry of Computed
fn parse_model_results_single(part: &[&str]) -> Result<Computed> {
    // collect records as header separated lines
    // blank lines are ignored
    let mut records: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut header = None;
    for line in part {
        let line = line.trim();
        if line.starts_with("@") {
            // header = line.split_whitespace().next();
            header = line.trim_end().into();
        } else {
            if let Some(k) = header {
                records.entry(k).or_insert(vec![]).push(line);
            }
        }
    }

    // parse record values
    if records.len() < 1 {
        warn!("Collected no results. Please check if the stream is clean!");
        warn!("suspicious part: {:?}", part);
    }

    let mut results = Computed::default();
    for (k, lines) in records {
        let header: Header = k.parse()?;
        let unit_factor = header.unit_factor;
        match header.name.as_str() {
            "energy" => {
                assert_eq!(1, lines.len(), "expect one line containing energy");
                let energy = lines[0].trim().parse::<f64>()? * unit_factor;
                results.energy = Some(energy);
            }
            "forces" => {
                let mut forces: Vec<[f64; 3]> = vec![];
                for line in lines {
                    let parts: Vec<_> = line.split_whitespace().collect();
                    if parts.len() != 3 {
                        bail!("expect xyz forces: {}", line);
                    }
                    let fx = parts[0].parse::<f64>()? * unit_factor;
                    let fy = parts[1].parse::<f64>()? * unit_factor;
                    let fz = parts[2].parse::<f64>()? * unit_factor;
                    forces.push([fx, fy, fz]);
                }

                results.forces = Some(forces);
            }
            "structure" => {
                let mut s = lines.join("\n");
                s.push_str("\n\n");
                let mol = Molecule::from_str(&s, "text/pxyz")?;
                results.molecule = Some(mol);
            }
            "dipole" => {
                assert_eq!(1, lines.len(), "expect one line containing dipole moment");
                let parts: Vec<_> = lines[0].split_whitespace().collect();
                let fx = parts[0].parse::<f64>()? * unit_factor;
                let fy = parts[1].parse::<f64>()? * unit_factor;
                let fz = parts[2].parse::<f64>()? * unit_factor;
                results.dipole = Some([fx, fy, fz]);
            }
            _ => {
                warn!("ignored record: {:?}", k);
            }
        }
    }

    Ok(results)
}

fn parse_model_results(stream: &str) -> Result<Vec<Computed>> {
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
        // ignore empty part
        if part.is_empty() {
            continue;
        } else {
            let mp = parse_model_results_single(part)?;
            all_results.push(mp);
        }
    }

    Ok(all_results)
}
// 37f15603 ends here

impl Computed {
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

    /// Set molecule structure.
    ///
    /// # Parameters
    ///
    /// * atoms: a list of symbol and position pairs
    ///
    /// * cell: three Lattice vectors array
    ///
    /// * scaled: indicates if input atom positions in scaled coordinates
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

// [[file:../models.note::6d51755f][6d51755f]]
#[test]
fn test_model_parse_results() {
    use vecfx::approx::*;

    use serde_json;

    let txt = gchemol::io::read_file("tests/files/sample.txt").unwrap();
    let r: Computed = txt.parse().expect("model results");

    // serializing
    let serialized = serde_json::to_string(&r).unwrap();
    // and deserializing
    let _: Computed = serde_json::from_str(&serialized).unwrap();

    // reformat
    let txt = format!("{}", r);

    // parse again
    let r: Computed = txt.parse().expect("model results");

    assert!(&r.molecule.is_some());
    let ref mol = r.molecule.unwrap();
    assert_eq!(3, mol.natoms());
    let e = r.energy.expect("model result: energy");
    assert_relative_eq!(-0.329336, e, epsilon = 1e-4);
}

#[test]
fn test_model_parse_results_special() -> Result<()> {

    let txt = gchemol::io::read_file("./tests/files/sample_special.txt")?;
    let r: Computed = txt.parse()?;
    assert_eq!(r.energy.unwrap(), 0.0);
    assert_eq!(r.forces.unwrap()[0][0], 0.10525500903260E-03);

    Ok(())
}
// 6d51755f ends here
