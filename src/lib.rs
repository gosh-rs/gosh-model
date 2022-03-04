// [[file:../models.note::4e128786][4e128786]]
use gosh_core::*;
use gut::prelude::*;
// 4e128786 ends here

// [[file:../models.note::*mods][mods:1]]
mod model_properties;

mod blackbox;
mod lj;
// mods:1 ends here

// [[file:../models.note::bf8cc73b][bf8cc73b]]
use gchemol::prelude::*;
use gchemol::Molecule;

/// Trait for chemical calculations
pub trait ChemicalModel {
    /// Define how to compute molecular properties, such as energy, forces, or
    /// structure ...
    fn compute(&mut self, mol: &Molecule) -> Result<Computed>;

    /// Define how to compute the properties of a bunch of molecules, mainly for
    /// reduce IO costs of small molecule calculations.
    fn compute_bunch(&mut self, _mols: &[Molecule]) -> Result<Vec<Computed>> {
        unimplemented!()
    }
}
// bf8cc73b ends here

// [[file:../models.note::616b7a47][616b7a47]]
pub use crate::blackbox::BlackBoxModel;
pub use crate::lj::LennardJones;
pub use crate::model_properties::*;

pub type BlackBox = BlackBoxModel;
pub type ModelProperties = Computed;
// 616b7a47 ends here
