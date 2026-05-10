cd ./VeriPB
cargo clean
cd ..
if [ -f veripb ]; then 
    rm veripb
fi
cd ./Satsuma
make clean
cd ..
if [ -f satsuma ]; then 
    rm satsuma
fi
make clean
if [ -f kissat ]; then 
    rm kissat
fi