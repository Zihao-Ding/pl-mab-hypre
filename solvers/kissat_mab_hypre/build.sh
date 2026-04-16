cd ./VeriPB
cargo build --release
cd ..
cp ./VeriPB/target/release/veripb ./
cd ./Satsuma
cmake .
make satsuma
cd ..
cp ./Satsuma/satsuma ./
./configure
make
cp ./build/kissat ./