// [[file:../models.note::178e12ff][178e12ff]]
use super::*;

use std::collections::{HashMap, HashSet};

use ::edip::EdipParameters;
use gchemol::neighbors::Neighborhood;
use vecfx::*;
// 178e12ff ends here

// [[file:../models.note::6e669f3b][6e669f3b]]
#[derive(Clone, Debug, Default)]
pub struct Edip {
    virial: f64,
    // for create neighbors
    nh: Neighborhood,
}

impl Edip {
    fn update_nh(&mut self, mol: &Molecule) {
        self.nh = Neighborhood::new();
        // use atom index (0-based) for node index
        self.nh.update(mol.positions().enumerate());
        if let Some(lat) = mol.get_lattice() {
            self.nh.set_lattice(lat.matrix().into());
        }
    }
}

impl ChemicalModel for Edip {
    fn compute(&mut self, mol: &Molecule) -> Result<Computed> {
        const search_radius: f64 = 4.0;

        // only works for silicon
        let not_silicon = mol.symbols().any(|x| x != "Si");
        if not_silicon {
            bail!("EDIP potential model only works for Silicon");
        }

        self.update_nh(mol);
        let n = mol.natoms();
        let positions = mol.positions().collect_vec();
        let mut neighbors = vec![];
        let mut distances = HashMap::new();
        // FIXME: rewrite for periodic system
        let lat = mol.get_lattice();
        for i in 0..n {
            let mut connected = HashSet::new();
            for x in self.nh.neighbors(i, search_radius) {
                // FIXME: avoid recompute pair distance in edip crate
                let j = x.node;
                let pi: Vector3f = positions[i].into();
                let pj: Vector3f = positions[j].into();
                let d = if let Some(image) = x.image {
                    // translation periodic image
                    let t = lat.unwrap().to_cart(image);
                    pj + t - pi
                } else {
                    pj - pi
                };
                distances.insert((i, j), d.into());
                connected.insert(j);
            }

            neighbors.push(connected);
        }

        let params = EdipParameters::silicon();
        let mut forces = vec![[0.0; 3]; n];
        let (energy, virial) = ::edip::compute_forces(&mut forces, &neighbors, &distances, &params);

        let mut computed = Computed::default();
        computed.set_energy(energy);
        computed.set_forces(forces);

        // FIXME: could be removed
        self.virial = virial;

        Ok(computed)
    }
}
// 6e669f3b ends here

// [[file:../models.note::28122508][28122508]]
#[test]
fn test_edip() -> Result<()> {
    use gchemol::prelude::*;
    use gchemol::Molecule;
    use vecfx::*;

    let f = "./tests/files/si10.xyz";
    let mol = Molecule::from_file(f)?;

    let mut model = Edip::default();
    let computed = model.compute(&mol)?;
    let f = computed.get_forces().unwrap();
    let f_norm = f.as_flat().as_vector_slice().norm();
    assert!(dbg!(f_norm) <= 0.005);
    let energy = computed.get_energy().unwrap();
    approx::assert_relative_eq!(energy, -39.57630354331939, epsilon = 1e-5);
    approx::assert_relative_eq!(model.virial, 0.00224989660978845, epsilon = 1e-5);

    let f = "./tests/files/si5.xyz";
    let mol = Molecule::from_file(f)?;
    let computed = model.compute(&mol)?;
    let energy = computed.get_energy().unwrap();
    approx::assert_relative_eq!(energy, -14.566606, epsilon = 1e-5);
    // FIXME: refactor
    let virial = model.virial;
    approx::assert_relative_eq!(virial, -3.643552, epsilon = 1e-5);

    let f = computed.get_forces().unwrap();
    #[rustfmt::skip]
    let f_expected = [ -0.19701000,  -0.62522600,   0.02948000,
                       -0.23330600,   0.43698600,   0.42511400,
                        0.43297600,   0.18333500,  -0.44864300,
                       -1.75669300,   0.50149400,  -1.48879300,
                        1.75403300,  -0.49658900,   1.48284200];
    approx::assert_relative_eq!(
        f.as_flat().as_vector_slice(),
        f_expected.as_vector_slice(),
        epsilon = 1E-5,
    );

    // for silicon crystal
    let mol = Molecule::from_file("./tests/files/si-3x3x3.cif")?;
    let computed = model.compute(&mol)?;
    let energy = computed.get_energy().unwrap();
    let f = computed.get_forces().unwrap();
    dbg!(f.as_flat().as_vector_slice().norm());

    Ok(())
}
// 28122508 ends here
