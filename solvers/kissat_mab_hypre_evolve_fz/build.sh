#!/bin/bash
cd ./VeriPB
cargo build --release
cd ..
cp ./VeriPB/target/release/veripb ./
cd ./Satsuma
make satsuma
cd ..
cp ./Satsuma/satsuma ./
./configure -static
cd build && make kissat
cd ..
cp ./build/kissat ./