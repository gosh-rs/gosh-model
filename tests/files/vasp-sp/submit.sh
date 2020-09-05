#! /usr/bin/env bash

## 0. load VASP related environment variables
source /share/apps/vasp/env.rc

## 1. Prepare input files required for VASP calculation
# Create POSCAR from standard input (stdin), formatted with user template
# (input.hbs)
cat > POSCAR

## Prepare other input files
# copy important files into the .tmp* scratch directory, which will be
# automatically removed if job finished successful.
cp "$BBM_JOB_DIR/INCAR" .
cp "$BBM_JOB_DIR/POTCAR" .
cp "$BBM_JOB_DIR/KPOINTS" .

# Try resume VASP calculation from last step.
cp "$BBM_JOB_DIR/WAVECAR" . 2>/dev/null
cp "$BBM_JOB_DIR/CHGCAR" . 2>/dev/null

## 2. How to run vasp
# PLEASE CHANGE
# submit vasp, ignoring stdout produced from vasp exe.
run-vasp.sh > "$BBM_JOB_DIR"/vasp.log

## 3. Post-processes
# save CHGCAR and WAVECAR for resuming calculation later
cp CHGCAR WAVECAR OUTCAR "$BBM_JOB_DIR/" 2>/dev/null
# save intermediate structures
cp OUTCAR CONTCAR "$BBM_JOB_DIR/" 2>/dev/null

## 4. extract energy and forces from OUTCAR to stdout
gosh-adaptor vasp OUTCAR
