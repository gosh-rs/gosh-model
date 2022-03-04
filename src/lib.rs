// [[file:../models.note::*common][common:1]]
pub(crate) mod core {
    pub use gosh_core::*;

    pub use gut::prelude::*;
}
// common:1 ends here

// [[file:../models.note::*mods][mods:1]]
mod model_properties;

mod blackbox;
mod lj;
// mods:1 ends here

// [[file:../models.note::bf8cc73b][bf8cc73b]]
use crate::core::*;

use gchemol::prelude::*;
use gchemol::Molecule;

/// Trait for chemical calculations
pub trait ChemicalModel {
    /// Define how to compute molecular properties, such as energy, forces, or
    /// structure ...
    fn compute(&mut self, mol: &Molecule) -> Result<ModelProperties>;

    /// Define how to compute the properties of a bunch of molecules, mainly for
    /// reduce IO costs of small molecule calculations.
    fn compute_bunch(&mut self, mols: &[Molecule]) -> Result<Vec<ModelProperties>> {
        unimplemented!()
    }
}
// bf8cc73b ends here

// [[file:../models.note::*pub][pub:1]]
pub use crate::blackbox::BlackBoxModel;
pub use crate::lj::LennardJones;
pub use crate::model_properties::*;

pub type BlackBox = BlackBoxModel;
// pub:1 ends here
