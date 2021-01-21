// [[file:../models.note::*imports][imports:1]]
use crate::core::*;
use crate::*;

use gut::prelude::*;

use std::path::{Path, PathBuf};
// imports:1 ends here

// [[file:../models.note::*update INCAR][update INCAR:1]]
pub fn update_vasp_incar_file(path: &Path) -> Result<()> {
    info!("Update INCAR for interactive calculation ...");

    // INCAR file may contains invalid UTF-8 characters, so we handle it using
    // byte string
    use bstr::{ByteSlice, B};

    let mandatory_params = vec![
        "POTIM = 0",
        "NELM = 200",
        // a large enough value is required
        "NSW = 99999",
        // need print energy and forces on each ion step
        "NWRITE = 1",
        "IBRION = -1",
        "ISYM = 0",
        // the key to enter interactive mode
        "INTERACTIVE = .TRUE.",
    ];

    // remove mandatory tags defined by user, so we can add the required
    // parameters later
    let bytes = std::fs::read(path).with_context(|| format!("read {:?}", path))?;
    let mut lines: Vec<&[u8]> = bytes
        .lines()
        .filter(|line| {
            let s = line.trim_start();
            if !s.starts_with_str("#") && s.contains_str("=") {
                let parts: Vec<_> = s.splitn_str(2, "=").collect();
                if parts.len() == 2 {
                    let tag = parts[0].trim().to_uppercase();
                    for param in mandatory_params.iter() {
                        let param = param.as_bytes().as_bstr();
                        if param.starts_with(&tag) {
                            return false;
                        }
                    }
                }
            }
            true
        })
        .collect();

    // append mandatory parameters
    lines.push(B("# Mandatory parameters for interactive VASP calculation:"));
    for param in mandatory_params.iter() {
        lines.push(B(param));
    }
    let txt = bstr::join("\n", &lines);

    // write it back
    gut::fs::write_to_file(path, &txt.to_str_lossy())?;

    Ok(())
}

#[test]
#[ignore]
fn test_update_incar() -> Result<()> {
    update_vasp_incar_file("./tests/files/INCAR".as_ref())?;

    Ok(())
}
// update INCAR:1 ends here

// [[file:../models.note::*poscar][poscar:1]]
// read scaled positions from POSCAR
fn get_scaled_positions_from_poscar(path: &Path) -> Result<String> {
    let s = gut::fs::read_file(path)?;

    let lines: Vec<_> = s
        .lines()
        .skip_while(|line| !line.to_uppercase().starts_with("DIRECT"))
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .collect();
    let mut positions = lines.join("\n");
    // final line separator
    positions += "\n";
    Ok(positions)
}

#[test]
#[ignore]
fn test_poscar_positions() -> Result<()> {
    let poscar = "./tests/files/live-vasp/POSCAR";

    let s = get_scaled_positions_from_poscar(poscar.as_ref())?;
    assert_eq!(s.lines().count(), 25);

    Ok(())
}
// poscar:1 ends here

// [[file:../models.note::*stopcar][stopcar:1]]
fn write_stopcar(dir: &Path) -> Result<()> {
    gut::fs::write_to_file(dir.join("STOPCAR"), "LABORT = .TRUE.\n").context("write STOPCAR")?;

    Ok(())
}
// stopcar:1 ends here

// [[file:../models.note::*stdout][stdout:1]]
pub(crate) mod stdout {
    use super::*;
    use std::io::prelude::*;
    use text_parser::parsers::*;

    fn parse_vasp_energy(s: &str) -> Option<f64> {
        if s.len() < 42 {
            None
        } else {
            s[26..26 + 16].trim().parse().ok()
        }
    }

    #[test]
    fn test_parse_vasp_energy() {
        let s = "   1 F= -.84780990E+02 E0= -.84775142E+02  d E =-.847810E+02  mag=     3.2666";
        let (_, e) = read_energy(s).unwrap();
        assert_eq!(e, -0.84775142E+02);
    }

    // FORCES:
    //      0.2084558     0.2221942    -0.1762308
    //     -0.1742340     0.2172782     0.2304866
    //      0.2244132    -0.1794341     0.2106465
    //     -0.2907316    -0.2746548    -0.2782190
    //     -0.2941880    -0.0306001    -0.0141722
    fn read_forces(s: &str) -> IResult<&str, Vec<[f64; 3]>> {
        let tag_forces = tag("FORCES:");
        let read_forces = many1(read_xyz);

        do_parse!(
            s,
            tag_forces >> eol   >>     // FORCES:
            forces: read_forces >>     // forces in each line
            (forces)
        )
    }

    //      0.2084558     0.2221942    -0.1762308
    fn read_xyz(s: &str) -> IResult<&str, [f64; 3]> {
        do_parse!(
            s,
            space1 >> xyz: xyz_array >> read_line >> // ignore the remaining characters
            (xyz)
        )
    }

    // RMM:   7    -0.593198855580E+03    0.91447E-04   -0.23064E-04   436   0.279E-02
    fn read_electron_step(s: &str) -> IResult<&str, usize> {
        let tag_rmm = tag("RMM:");
        do_parse!(
            s,
            tag_rmm >> space1 >> step: unsigned_digit >> read_line >> // ignore the remaining characters
            (step)
        )
    }

    //    1 F= -.85097948E+02 E0= -.85096866E+02  d E =-.850979E+02  mag=     2.9646
    //    2 F= -.85086257E+02 E0= -.85082618E+02  d E =-.850863E+02  mag=     2.9772
    // POSITIONS: reading from stdin
    fn read_energy(s: &str) -> IResult<&str, f64> {
        let tag_nf = tag("F=");
        let tag_e0 = tag("E0=");
        do_parse!(
            s,
            space0 >> digit1 >> space1 >> tag_nf >> space0 >> double >>  // 1 F= ...
            space0 >> tag_e0 >> space0 >> energy: double >> read_line >> // E0= ...
            (energy)
        )
    }

    fn read_energy_and_forces(s: &str) -> IResult<&str, (f64, Vec<[f64; 3]>)> {
        let jump = take_until("FORCES:\n");
        do_parse!(
            s,
            jump >>                       // skip leading text until found "FORCES"
            forces: read_forces        >> // read forces
            energy: read_energy        >> // read forces
            ((energy, forces))
        )
    }

    /// Parse energy and forces from stdout of VASP interactive calculation
    pub fn parse_energy_and_forces(s: &str) -> Result<(f64, Vec<[f64; 3]>)> {
        // HACK: show SCF steps for references
        let lines = s.lines().collect_vec();
        let pos = lines
            .iter()
            .position(|line| line.starts_with("FORCES:"))
            .expect("vasp stdout: FORCES");
        // the line above "FORCES:"
        // DAV:  22    -0.870187496108E+03   -0.18821E+00   -0.15766E-01 20316   0.597E-01    0.108E+00
        // or
        // RMM:  22    -0.870187496108E+03   -0.18821E+00   -0.15766E-01 20316   0.597E-01    0.108E+00
        debug!("{}", lines[pos - 1]);
        let (_, values) = read_energy_and_forces(s).expect("parse energy/forces from vasp stdout");
        Ok(values)
    }

    #[test]
    #[ignore]
    fn test_parse_vasp_interactive() -> Result<()> {
        let s = "./tests/files/interactive.txt";
        let s = gut::fs::read_file(s)?;

        let (e, f) = parse_energy_and_forces(&s)?;
        assert_eq!(f.len(), 25);

        Ok(())
    }
}
// stdout:1 ends here
