#! /usr/bin/env bash

# 0. Load SIESTA related environment variables
# Please change if necessary
# require gosh >= 0.0.25
# parent_dir=$(realpath "$BBM_TPL_DIR/..")
parent_dir=/share/apps/siesta
source "$parent_dir/env.rc"

# 1. prepare SIESTA input files
ln -s "$parent_dir"/psml/* . 2>/dev/null
ln -s "$BBM_TPL_DIR"/default.fdf . 2>/dev/null

# Make SIESTA resume from last job. This could reduce electronic step costs.
cp "$BBM_JOB_DIR/siesta.DM" . 2>/dev/null

# 2. read fdf stream from stdin, write stdout to siesta.log for parsing
tee input.fdf | mpirun -np 12 2>&1 | tee siesta.log > "$BBM_JOB_DIR"/siesta.log

# 3. save important files to bbm job starting dir
# require gosh >= 0.0.25
cp siesta.DM "$BBM_JOB_DIR/" 2>/dev/null

# 4. parse calculated results
gosh-adaptor siesta siesta.log
