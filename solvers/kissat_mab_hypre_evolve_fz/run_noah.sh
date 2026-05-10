#!/bin/bash

if [ $# -lt 4 ]; then
    echo "Usage: $0 <satsuma_path> <kissat_path> <input_file(.cnf|.cnf.xz)> <time_limit>"
    exit 1
fi

satsuma_path="$1"
kissat_path="$2"
input_file="$3"
time_limit="$4"

if [ ! -x "$satsuma_path" ]; then
    echo "Error: satsuma not found or not executable: $satsuma_path"
    exit 1
fi

if [ ! -x "$kissat_path" ]; then
    echo "Error: kissat not found or not executable: $kissat_path"
    exit 1
fi

if [ ! -f "$input_file" ]; then
    echo "Error: file not found: $input_file"
    exit 1
fi

# 判断是否为 xz 压缩
is_xz=0
if [[ "$input_file" == *.xz ]]; then
    is_xz=1
fi

# 读取 p cnf 行中的变量数 N
N=""
if [ "$is_xz" -eq 1 ]; then
    while IFS= read -r line; do
        if [[ $line == p\ * ]]; then
            read -r _ _ N _ <<< "$line"
            break
        fi
    done < <(xz -dc -- "$input_file")
else
    while IFS= read -r line; do
        if [[ $line == p\ * ]]; then
            read -r _ _ N _ <<< "$line"
            break
        fi
    done < "$input_file"
fi

if [ -z "$N" ]; then
    echo "Error: failed to find 'p cnf' line in $input_file"
    exit 1
fi

echo "变量数为 $N"

input_basename=$(basename "$input_file")

# 如果输入是 .cnf.xz，给求解器准备一个临时解压文件
solver_input="$input_file"
decompressed_tmp=""
temporary_cnf=""

cleanup() {
    [ -n "$temporary_cnf" ] && [ -f "$temporary_cnf" ] && rm -f "$temporary_cnf"
    [ -n "$decompressed_tmp" ] && [ -f "$decompressed_tmp" ] && rm -f "$decompressed_tmp"
}
trap cleanup EXIT

if [ "$is_xz" -eq 1 ]; then
    decompressed_tmp=$(mktemp --suffix=.cnf)
    xz -dc -- "$input_file" > "$decompressed_tmp"
    solver_input="$decompressed_tmp"
fi

# 生成中间 CNF 文件名
base_no_xz="${input_basename%.xz}"
temporary_cnf="${base_no_xz}.temporary.cnf"

if [ "$N" -lt 5000000 ]; then
    echo "call satsuma"
    "$satsuma_path" \
        --proof-dense-crossover 60 \
        --component-limit 500000 \
        --order-model-limit 750000 \
        --dense-model-limit 20000000 \
        --add-reduced-as-unit \
        --preprocess-cnf-unit \
        -f "$solver_input" \
        --out-file "$temporary_cnf"
    SATSUMA_EXIT=$?
    if [ "$SATSUMA_EXIT" -eq 0 ]; then
        # satsuma 仅预处理，交给 kissat
        "$kissat_path" "$temporary_cnf" --time="$time_limit" --no-binary
    elif [ "$SATSUMA_EXIT" -eq 1 ]; then
        echo "s UNSATISFIABLE (by satsuma)"
        exit 20
    else
        echo "s UNKNOWN (satsuma exit=$SATSUMA_EXIT)"
        exit 0
    fi
else
    echo "don't call satsuma"
    "$kissat_path" "$solver_input" --time="$time_limit" --no-binary
fi