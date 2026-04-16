cd ./VeriPB
cargo clean
cd ..
if [ -f veripb ]; then 
    rm veripb
fi
cd ./Satsuma
cd ./dejavu
rm -rf CMakeFiles CMakeCache.txt Makefile cmake_install.cmake
if [ -f dejavu ]; then
    rm dejavu
fi
cd ..
rm -rf CMakeFiles CMakeCache.txt Makefile cmake_install.cmake
if [ -f satsuma ]; then
    rm satsuma
fi
cd ..
if [ -f satsuma ]; then 
    rm satsuma
fi
make clean
if [ -f kissat ]; then 
    rm kissat
fi