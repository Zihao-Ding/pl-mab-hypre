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
proof_file="${input_basename}.proof.out"

echo "pseudo-Boolean proof version 3.0" > "$proof_file"
./kissat "$input_file" "$proof_file" --time=5000 --no-binary -q > /dev/null 2>&1
KISSAT_EXIT=$?
echo "output NONE;" >> "$proof_file"
echo "conclusion UNSAT;" >> "$proof_file"
echo "end pseudo-Boolean proof;" >> "$proof_file"

if [ $KISSAT_EXIT -eq 20 ]; then
    echo "s UNSATISFIABLE  ${input_basename}"
elif [ $KISSAT_EXIT -eq 10 ]; then
    echo "s SATISFIABLE    ${input_basename}"
else
    echo "s UNKNOWN        ${input_basename}"
fi

if [ -f "$proof_file" ]; then 
    rm "$proof_file"
fi