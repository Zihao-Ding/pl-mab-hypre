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
sat_file="${input_basename}.sat"
proof_file="${input_basename}.proof.out"

SATSUMA_EXIT=0
if [ "$N" -lt 5000000 ]; then
    ./satsuma --proof-dense-crossover 60 --component-limit 500000 --order-model-limit 750000 --dense-model-limit 20000000 --add-reduced-as-unit --preprocess-cnf-unit -f "$input_file" --proof-file "$proof_file" --out-file "$temporary_cnf" > /dev/null 2>&1
    SATSUMA_EXIT=$?
    if [ $SATSUMA_EXIT -eq 0 ]; then
        ./kissat "$temporary_cnf" "$proof_file" --time=5000 --no-binary -q 2>&1 | tee >(grep '^v' > "$sat_file") | cat > /dev/null 2>&1
    fi
else
    echo "pseudo-Boolean proof version 3.0" > "$proof_file"
    ./kissat "$input_file" "$proof_file" --time=5000 --no-binary -q 2>&1 | tee >(grep '^v' > "$sat_file") | cat > /dev/null 2>&1
fi

KISSAT_EXIT=${PIPESTATUS[0]}
echo "output NONE;" >> "$proof_file"
echo "conclusion UNSAT;" >> "$proof_file"
echo "end pseudo-Boolean proof;" >> "$proof_file"
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

if [ $SATSUMA_EXIT -eq 1 ]; then
    timeout 45000 ./veripb "$input_file" "$proof_file" --stats > /dev/null 2>&1
    DRAT_EXIT=$?
    if [ $DRAT_EXIT -eq 0 ]; then
        echo "s VERIFIED       ${input_basename}"
    else
        echo "s UNVERIFIED     ${input_basename}"
    fi
elif [ $KISSAT_EXIT -eq 20 ]; then
    timeout 45000 ./veripb "$input_file" "$proof_file" --stats > /dev/null 2>&1
    DRAT_EXIT=$?
    if [ $DRAT_EXIT -eq 0 ]; then
        echo "s VERIFIED       ${input_basename}"
    else
        echo "s UNVERIFIED     ${input_basename}"
    fi
elif [ $KISSAT_EXIT -eq 10 ]; then
    python3 verifysat.py "$input_file" "$sat_file" > /dev/null 2>&1
    SAT_EXIT=$?
    if [ $SAT_EXIT -eq 0 ]; then
        echo "s VERIFIED       ${input_basename}"
    else
        echo "s UNVERIFIED     ${input_basename}"
    fi
fi

if [ -f "$sat_file" ]; then 
    rm "$sat_file"
fi

if [ -f "$proof_file" ]; then 
    rm "$proof_file"
fi