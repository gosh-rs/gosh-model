// header

// [[file:~/Workspace/Programming/gosh-rs/model/models.note::*header][header:1]]
//! The Lennard-Jones model for test purpose
// header:1 ends here

// imports

// [[file:~/Workspace/Programming/gosh-rs/model/models.note::*imports][imports:1]]
use crate::core::*;

use gchemol::Molecule;
use vecfx::*;

use crate::*;
// imports:1 ends here

// core

// [[file:~/Workspace/Programming/gosh-rs/model/models.note::*core][core:1]]
#[derive(Clone, Copy, Debug)]
pub struct LennardJones {
    /// Energy constant of the Lennard-Jones potential
    pub epsilon: f64,
    /// Distance constant of the Lennard-Jones potential
    pub sigma: f64,

    pub derivative_order: usize,
}

impl Default for LennardJones {
    fn default() -> Self {
        LennardJones {
            epsilon: 1.0,
            sigma: 1.0,
            // energy only
            derivative_order: 0,
        }
    }
}

impl LennardJones {
    // vij
    fn pair_energy(&self, r: f64) -> f64 {
        let s6 = f64::powi(self.sigma / r, 6);
        4.0 * self.epsilon * (f64::powi(s6, 2) - s6)
    }

    // dvij
    fn pair_gradient(&self, r: f64) -> f64 {
        let s6 = f64::powi(self.sigma / r, 6);

        24.0 * self.epsilon * (s6 - 2.0 * f64::powi(s6, 2)) / r
    }

    /// Evaluate energy and forces
    pub fn evaluate(&self, positions: &[[f64; 3]], forces: &mut [[f64; 3]]) -> f64 {
        let n = positions.len();
        debug_assert_eq!(n, forces.len(), "positions.len() != forces.len()");

        // initialize with zeros
        for i in 0..n {
            for j in 0..3 {
                forces[i][j] = 0.0;
            }
        }

        // collect parts in parallel
        let parts: Vec<_> = (0..n)
            .into_par_iter()
            .flat_map(|i| {
                (0..i).into_par_iter().map(move |j| {
                    let r = positions[i].vecdist(&positions[j]);
                    let e = self.pair_energy(r);
                    let g = self.pair_gradient(r) / r;
                    (e, g, (i, j))
                })
            })
            .collect();

        // calculate energy
        let energy: f64 = parts.iter().map(|(e, _, _)| *e).sum();

        // calculate force
        for (_, g, (i, j)) in parts {
            for k in 0..3 {
                let dr = positions[j][k] - positions[i][k];
                forces[i][k] += 1.0 * g * dr;
                forces[j][k] += -1.0 * g * dr;
            }
        }

        energy
    }
}
// core:1 ends here

// entry

// [[file:~/Workspace/Programming/gosh-rs/model/models.note::*entry][entry:1]]
impl ChemicalModel for LennardJones {
    fn compute(&mut self, mol: &Molecule) -> Result<ModelProperties> {
        if mol.lattice.is_some() {
            warn!("LJ model: periodic lattice will be ignored!");
        }

        let natoms = mol.natoms();
        let mut energy = 0.0;
        let mut forces = Vec::with_capacity(natoms);

        // initialize with zeros
        for _ in 0..natoms {
            forces.push([0.0; 3]);
        }

        // calculate energy and forces
        let positions: Vec<_> = mol.positions().collect();
        let dm = gchemol::geom::get_distance_matrix(&positions);
        for i in 0..natoms {
            for j in 0..i {
                let r = dm[i][j];
                energy += self.pair_energy(r);
                if self.derivative_order >= 1 {
                    let g = self.pair_gradient(r);
                    for k in 0..3 {
                        let dr = positions[j][k] - positions[i][k];
                        forces[i][k] += 1.0 * g * dr / r;
                        forces[j][k] += -1.0 * g * dr / r;
                    }
                }
            }
        }

        let mut mr = ModelProperties::default();
        mr.set_energy(energy);

        if self.derivative_order >= 1 {
            mr.set_forces(forces);
        }
        if self.derivative_order >= 2 {
            unimplemented!();
        }

        Ok(mr)
    }
}
// entry:1 ends here

// test

// [[file:~/Workspace/Programming/gosh-rs/model/models.note::*test][test:1]]
#[test]
fn test_lj_model() {
    use approx::*;

    let mut lj = LennardJones::default();
    lj.derivative_order = 1;

    // LJ3
    let mol = Molecule::from_file("tests/files/LennardJones/LJ3.xyz").expect("lj3 test file");
    let mr = lj.compute(&mol).expect("lj model: LJ3");
    let e = mr.get_energy().expect("lj model energy: LJ3");
    assert_relative_eq!(-3.0, e, epsilon = 1e-3);

    let forces = mr.get_forces().expect("lj model forces: LJ3");
    for i in 0..mol.natoms() {
        for j in 0..3 {
            assert_relative_eq!(0.0, forces[i][j], epsilon = 1e-3);
        }
    }

    // LJ38
    let mol = Molecule::from_file("tests/files/LennardJones/LJ38.xyz").expect("lj38 test file");
    let mr = lj.compute(&mol).expect("lj model: LJ38");
    let e = mr.get_energy().expect("lj model energy: LJ38");
    assert_relative_eq!(-173.92843, e, epsilon = 1e-3);

    let forces = mr.get_forces().expect("lj model forces: LJ3");
    for i in 0..mol.natoms() {
        for j in 0..3 {
            assert_relative_eq!(0.0, forces[i][j], epsilon = 1e-3);
        }
    }
}
// test:1 ends here
