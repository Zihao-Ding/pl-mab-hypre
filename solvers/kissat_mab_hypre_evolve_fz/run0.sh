#!/bin/bash

if [ $# -lt 1 ]; then
    echo "Usage: $0 <input_file>"
    exit 1
fi
input_file="$1"

while IFS= read -r line; do
    if [[ $line == p\ * ]]; then
        read -r _ _ N _ <<< "$line"
        break
    fi
done < "$1"

input_basename=$(basename "$input_file")
temporary_cnf="${input_basename}.temporary.cnf"
SATSUMA_EXIT=0
if [ "$N" -lt 5000000 ]; then
    ./satsuma --proof-dense-crossover 60 --component-limit 500000 --order-model-limit 750000 --dense-model-limit 20000000 --add-reduced-as-unit --preprocess-cnf-unit -f "$input_file" --out-file "$temporary_cnf" > /dev/null 2>&1
    SATSUMA_EXIT=$?
    if [ $SATSUMA_EXIT -eq 0 ]; then
        ./kissat "$temporary_cnf" --time=5000 -q > /dev/null 2>&1
    fi
else
    ./kissat "$input_file" --time=5000 -q > /dev/null 2>&1
fi
KISSAT_EXIT=$?

if [ $SATSUMA_EXIT -eq 1 ]; then
    echo "s UNSATISFIABLE  ${input_basename}"
elif [ $KISSAT_EXIT -eq 20 ]; then
    echo "s UNSATISFIABLE  ${input_basename}"
elif [ $KISSAT_EXIT -eq 10 ]; then
    echo "s SATISFIABLE    ${input_basename}"
else
    echo "s UNKNOWN        ${input_basename}"
fi

if [ -f "$temporary_cnf" ]; then 
    rm "$temporary_cnf"
fi