#!/bin/bash

# ==================================
#     Script from Mallob 2023
# ==================================

MAX_N_SOLVERS_PER_PROCESS=32
MAX_N_SBVA=12
MAX_LS=2

# export OMPI_MCA_btl_vader_single_copy_mechanism=none

# Number of threads per MPI process: Set to available hardware threads,
# but at most $MAX_N_SOLVERS_PER_PROCESS.
n_threads_per_process=$(lscpu | awk '/^Core\(s\) per socket:/ { cores_per_socket = $NF } /^Socket\(s\):/ { sockets = $NF } END { print cores_per_socket * sockets }')
n_sbva_per_process=$MAX_N_SBVA
n_ls_after_sbva=$MAX_LS

if [ $n_threads_per_process -gt $MAX_N_SOLVERS_PER_PROCESS ]; then
    n_threads_per_process=$MAX_N_SOLVERS_PER_PROCESS
fi

if [ $n_threads_per_process -le $MAX_N_SBVA ]; then
    n_sbva_per_process=1
    n_ls_after_sbva=0
fi

echo "n_threads_per_process ${n_threads_per_process}"
echo "n_sbva_per_process ${n_sbva_per_process}"
echo "n_ls_after_sbva ${n_ls_after_sbva}"

verbosity=1

nglobalprocs=$(cat $2|wc -l)
echo "Running Painless with $n_threads_per_process threads on $(hostname) as leader and with $nglobalprocs MPI processes in total"

# nglobalprocs=1

nb_solvers=$(($n_threads_per_process - 1))

# start solving process
while IFS= read -r line; do
    if [[ $line == p\ * ]]; then
        read -r _ _ N _ <<< "$line"
        break
    fi
done < "$1"
temp_cnf="$1.temp.cnf"
echo "temporary file name: $temp_cnf"

if [ "$N" -lt 500000 ]; then
    # satsuma setup
    echo "SATSUMA SETUP"
    command="./satsuma --proof-dense-crossover 60 --component-limit 500000 --order-model-limit 750000 --dense-model-limit 20000000 --add-reduced-as-unit --preprocess-cnf-unit -f "$1" --out-file "$temp_cnf""
    echo "EXECUTING: $command"
    eval $command
    SATSUMA_EXIT=$?
    if [ $SATSUMA_EXIT -eq 0 ]; then
        # parallel setup
        echo "PARALLEL SETUP: "
        # command="./painless_release -v=$verbosity -c=$nb_solvers -solver=Z -t=1000 -shr-strat=1 -shr-sleep=100000 -prs -gshr-strat=-1 $temp_cnf"
        
        # command="mpirun --mca btl_tcp_if_include eth0 --mca orte_abort_on_non_zero_status false --allow-run-as-root --hostfile $1 --bind-to none ./painless -v=$verbosity -c=$n_threads_per_process -solver=k -t=1000 -sbva-timeout=120 -shr-strat=1 -shr-sleep=100000 -gshr-strat=2 -dist $2"

        command="mpirun --mca btl_tcp_if_include eth0 --mca orte_abort_on_non_zero_status false --allow-run-as-root --hostfile $2 --bind-to none ./painless_release -v=$verbosity -c=$n_threads_per_process -solver=Z -t=1000 -sbva-timeout=120 -shr-strat=1 -shr-sleep=100000 -gshr-strat=2 -dist $temp_cnf"
        echo "EXECUTING: $command"
        eval $command
    else
        if [ $SATSUMA_EXIT -eq 1 ]; then
            echo "s UNSATISFIABLE"
        fi
    fi
    if [ -f "$temp_cnf" ]; then
        rm "$temp_cnf"
    fi
else
    # parallel setup
    echo "PARALLEL SETUP: "
    # command="./painless_release -v=$verbosity -c=$nb_solvers -solver=Z -t=1000 -shr-strat=1 -shr-sleep=100000 -prs -gshr-strat=-1 $1"

    command="mpirun --mca btl_tcp_if_include eth0 --mca orte_abort_on_non_zero_status false --allow-run-as-root --hostfile $2 --bind-to none ./painless_release -v=$verbosity -c=$n_threads_per_process -solver=Z -t=1000 -sbva-timeout=120 -shr-strat=1 -shr-sleep=100000 -gshr-strat=2 -dist $1"
    echo "EXECUTING: $command"
    eval $command
fi

