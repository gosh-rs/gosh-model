// common

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*common][common:1]]
pub(crate) mod common {
    pub use guts::prelude::*;
}
// common:1 ends here

// imports

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*imports][imports:1]]
use common::*;

use gchemol::Molecule;
use gchemol::prelude::*;
// imports:1 ends here

// mods

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*mods][mods:1]]
mod model_properties;

pub mod blackbox;
pub mod lj;

pub use crate::blackbox::BlackBox;
pub use crate::lj::LennardJones;
pub use crate::model_properties::*;
// mods:1 ends here

impl ModelProperties {
    /// Set item energy.
    pub fn set_energy(&mut self, e: f64) {
        assert!(e.is_sign_positive(), "invalid energy: {}", e);
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

    pub fn energy(&self) -> Option<f64> {
        self.energy
    }

    pub fn dipole(&self) -> Option<[f64; 3]> {
        self.dipole
    }

    pub fn forces(&self) -> Option<&Vec<[f64; 3]>> {
        self.forces.as_ref()
    }

    pub fn force_constants(&self) -> Option<&Vec<[f64; 3]>> {
        self.force_constants.as_ref()
    }
}

// chemical model

// [[file:~/Workspace/Programming/gosh-rs/models/models.note::*chemical model][chemical model:1]]
pub trait ChemicalModel {
    /// Define how to compute molecular properties, such as energy, forces, or
    /// structure ...
    fn compute(&mut self, mol: &Molecule) -> Result<ModelProperties>;

    #[deprecated(note = "use compute_bunch instead")]
    fn compute_bundle(&mut self, mols: &[Molecule]) -> Result<Vec<ModelProperties>> {
        self.compute_bunch(mols)
    }

    /// Define how to compute the properties of many molecules in bunch to
    /// reduce IO costs, especially useful for small molecules.
    fn compute_bunch(&mut self, mols: &[Molecule]) -> Result<Vec<ModelProperties>> {
        unimplemented!()
    }
}
// chemical model:1 ends here
