// [[file:~/Workspace/Programming/gosh-rs/model/models.note::*common][common:1]]
pub(crate) mod core {
    pub use gosh_core::*;

    pub use gut::prelude::*;
}
// common:1 ends here

// [[file:~/Workspace/Programming/gosh-rs/model/models.note::*mods][mods:1]]
mod model_properties;

mod blackbox;
mod lj;

pub use crate::blackbox::BlackBox;
pub use crate::lj::LennardJones;
pub use crate::model_properties::*;
pub type BlackBoxModel = BlackBox;
// mods:1 ends here

// [[file:~/Workspace/Programming/gosh-rs/model/models.note::*chemical model][chemical model:1]]
use crate::core::*;

use gchemol::prelude::*;
use gchemol::Molecule;

pub trait ChemicalModel {
    /// Define how to compute molecular properties, such as energy, forces, or
    /// structure ...
    fn compute(&mut self, mol: &Molecule) -> Result<ModelProperties>;

    #[deprecated(note = "use compute_bunch instead")]
    fn compute_bundle(&mut self, mols: &[Molecule]) -> Result<Vec<ModelProperties>> {
        self.compute_bunch(mols)
    }

    /// Define how to compute the properties of a bunch of molecules, mainly for
    /// reduce IO costs of small molecule calculations.
    fn compute_bunch(&mut self, mols: &[Molecule]) -> Result<Vec<ModelProperties>> {
        unimplemented!()
    }
}
// chemical model:1 ends here
